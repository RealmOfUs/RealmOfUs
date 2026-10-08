//! Content-bound catalog roots and owner pages; hosts supply bytes, never trusted paths.
use crate::{
    Result,
    documents::{Descriptor, Preview, PreviewEntity, document, hash_shape},
    error::require,
    formats, json, limits,
    spatial::{Profile, chunk_key, parse_chunk},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

document!(
    /// Identity and owned-entity count of one page, bound by the root catalog.
    PageDescriptor {
        /// Exactly `chunks/<canonical chunk key>.json`.
        page: String,
        /// Digest of the page's actual encoded bytes.
        sha256: String,
        /// Encoded byte count, at most the ordinary JSON budget.
        #[serde(deserialize_with = "json::unsigned")]
        bytes: u64,
        /// Number of entities owned by the page; foreign-only pages have zero.
        entities: usize,
    }
);
impl PageDescriptor {
    /// Check digest shape and the independent per-page budgets.
    pub fn validate_shape(&self) -> Result<()> {
        hash_shape(&self.sha256)?;
        require(
            self.bytes > 0
                && self.bytes <= limits::JSON_BYTES as u64
                && self.entities <= limits::PAGE_RECORDS,
            "catalog_budget",
            "catalog page budget exceeded",
        )
    }
}

document!(
    /// Small versioned root; absent chunk keys are known to contain no catalog entries.
    Manifest {
        /// Must equal the current preview format.
        format: String,
        /// Immutable spatial rules.
        profile: Profile,
        /// Canonical profile identity.
        profile_hash: String,
        /// Total owned entities, equal to the sum of page entity counts.
        entity_count: usize,
        /// Nonempty owner or coverage pages, keyed by canonical chunk coordinates.
        chunks: BTreeMap<String, PageDescriptor>,
    }
);
impl Manifest {
    /// Check the root format, count consistency, profile and all page paths.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            self.format == formats::PREVIEW,
            "invalid_format",
            "unsupported preview format",
        )?;
        self.profile.validate()?;
        hash_shape(&self.profile_hash)?;
        require(
            self.profile_hash == json::hash(&self.profile)?,
            "hash_mismatch",
            "profile hash mismatch",
        )?;
        require(
            self.chunks.len() <= limits::WORLD_CHUNKS,
            "catalog_budget",
            "too many catalog pages",
        )?;
        let mut count = 0_usize;
        for (key, descriptor) in &self.chunks {
            descriptor.validate_shape()?;
            let chunk = parse_chunk(key)?;
            require(
                (0..3).all(|a| {
                    chunk[a] >= self.profile.min_chunk[a] && chunk[a] < self.profile.max_chunk[a]
                }),
                "outside_world",
                "catalog chunk outside world",
            )?;
            require(
                descriptor.page == format!("chunks/{key}.json"),
                "unsafe_path",
                "noncanonical page path",
            )?;
            count = count
                .checked_add(descriptor.entities)
                .ok_or_else(|| crate::Error::new("catalog_budget", "entity count overflow"))?;
        }
        require(
            count == self.entity_count,
            "invalid_index",
            "root entity count differs from pages",
        )
    }
    /// Validate a root without reading page bytes; pages remain explicitly unavailable.
    pub fn validate(&self) -> Result<()> {
        self.validate_shape()?;
        require(
            json::canonical(self)?.len() <= limits::JSON_BYTES,
            "json_budget",
            "catalog root byte budget exceeded",
        )
    }

    /// Verify a requested page against its root-bound identity and local spatial rules.
    pub fn load_page(&self, key: &str, bytes: &[u8]) -> Result<Page> {
        let descriptor = self
            .chunks
            .get(key)
            .ok_or_else(|| crate::Error::new("invalid_index", "page absent from catalog root"))?;
        Descriptor {
            sha256: descriptor.sha256.clone(),
            bytes: descriptor.bytes,
        }
        .verify(bytes)?;
        let page: Page = json::parse(bytes)?;
        require(
            page.entities.len() == descriptor.entities,
            "invalid_index",
            "page entity count mismatch",
        )?;
        let mut ids = BTreeSet::new();
        for entity in &page.entities {
            entity.validate_shape()?;
            entity.textures.validate(crate::paths::OBJECTS)?;
            let (owner, offset) = self.profile.owner(entity.position_mm)?;
            require(
                chunk_key(owner) == key
                    && owner == entity.owner_chunk
                    && offset == entity.offset_mm,
                "invalid_placement",
                "entity belongs to a different page or offset",
            )?;
            self.profile.intersections(&entity.bounds_mm)?;
            for asset in [&entity.asset, &entity.lod1] {
                require(
                    asset.path == crate::paths::object(&asset.sha256),
                    "unsafe_path",
                    "invalid content-addressed asset path",
                )?;
            }
            require(
                ids.insert(&entity.id),
                "duplicate_id",
                "duplicate page entity",
            )?;
        }
        for reference in &page.foreign {
            crate::paths::identity(&reference.id)?;
            require(
                reference.owner != key
                    && self
                        .chunks
                        .get(&reference.owner)
                        .is_some_and(|p| p.entities > 0),
                "invalid_index",
                "foreign reference has no owner page",
            )?;
            require(
                ids.insert(&reference.id),
                "duplicate_id",
                "duplicate page reference",
            )?;
        }
        Ok(page)
    }
}

/// A lightweight reference to geometry crossing a chunk boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Foreign {
    /// Stable entity ID whose complete record appears on the owner page.
    pub id: String,
    /// Canonical owner chunk key.
    pub owner: String,
}

document!(
    /// One owner/coverage page, identified by the root rather than a second format tag.
    Page {
        /// Complete owned records, sorted by entity ID.
        entities: Vec<PreviewEntity>,
        /// Foreign intersections, sorted by entity ID.
        foreign: Vec<Foreign>,
    }
);
impl Page {
    /// Enforce the per-page entity/reference budget and canonical record order.
    pub fn validate_shape(&self) -> Result<()> {
        require(
            !self.entities.is_empty() || !self.foreign.is_empty(),
            "invalid_index",
            "empty pages must be omitted",
        )?;
        require(
            self.entities.len() + self.foreign.len() <= limits::PAGE_RECORDS,
            "catalog_budget",
            "catalog page record budget exceeded",
        )?;
        require(
            self.entities.windows(2).all(|p| p[0].id < p[1].id)
                && self.foreign.windows(2).all(|p| p[0].id < p[1].id),
            "duplicate_id",
            "page IDs must be unique and sorted",
        )
    }
}

/// A complete publication plan. Page strings preserve the bytes bound by the root.
#[derive(Debug, Clone, Serialize)]
pub struct Bundle {
    /// Root catalog to write as realm.json.
    pub root: Manifest,
    /// Page path to exact UTF-8 bytes, including the final newline.
    pub pages: BTreeMap<String, String>,
}
impl Preview {
    /// Page a validated complete projection, retaining each full record only on its owner.
    pub fn bundle(&self) -> Result<Bundle> {
        self.validate()?;
        let mut pages: BTreeMap<String, Page> = BTreeMap::new();
        for entity in &self.entities {
            let owner = chunk_key(entity.owner_chunk);
            pages
                .entry(owner.clone())
                .or_insert_with(|| Page {
                    entities: Vec::new(),
                    foreign: Vec::new(),
                })
                .entities
                .push(entity.clone());
            for chunk in self.profile.intersections(&entity.bounds_mm)? {
                let key = chunk_key(chunk);
                if key != owner {
                    pages
                        .entry(key)
                        .or_insert_with(|| Page {
                            entities: Vec::new(),
                            foreign: Vec::new(),
                        })
                        .foreign
                        .push(Foreign {
                            id: entity.id.clone(),
                            owner: owner.clone(),
                        });
                }
            }
        }
        let mut root = Manifest {
            format: formats::PREVIEW.into(),
            profile: self.profile.clone(),
            profile_hash: self.profile_hash.clone(),
            entity_count: self.entities.len(),
            chunks: BTreeMap::new(),
        };
        let mut encoded = BTreeMap::new();
        for (key, mut page) in pages {
            page.entities.sort_by(|a, b| a.id.cmp(&b.id));
            page.foreign.sort_by(|a, b| a.id.cmp(&b.id));
            page.validate_shape()?;
            let bytes = json::pretty(&page)?;
            let path = format!("chunks/{key}.json");
            root.chunks.insert(
                key,
                PageDescriptor {
                    page: path.clone(),
                    sha256: json::digest(&bytes),
                    bytes: bytes.len() as u64,
                    entities: page.entities.len(),
                },
            );
            encoded.insert(path, String::from_utf8(bytes).expect("JSON is UTF-8"));
        }
        root.validate()?;
        require(
            json::pretty(&root)?.len() <= limits::JSON_BYTES,
            "json_budget",
            "catalog root byte budget exceeded",
        )?;
        Ok(Bundle {
            root,
            pages: encoded,
        })
    }
}
