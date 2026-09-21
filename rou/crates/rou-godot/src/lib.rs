//! GDExtension host adapter; all domain operations use the same pure table as C.
use godot::{
    builtin::{VarArray, VarDictionary, Variant},
    prelude::*,
};
use rou_core::{formats, ops, viewer};
use serde_json::Value;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct RealmExtension;
// SAFETY: Godot invokes this adapter on its main thread. It holds no engine
// objects globally and passes only live PackedByteArray slices to the pure core.
#[gdextension]
unsafe impl ExtensionLibrary for RealmExtension {}

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
struct Rou {
    catalog: Option<viewer::Catalog>,
}

#[godot_api]
impl Rou {
    #[func]
    fn abi_version(&self) -> i64 {
        i64::from(formats::ABI_VERSION)
    }

    #[func]
    fn invoke(&self, op: GString, input: PackedByteArray) -> VarDictionary {
        envelope(|| ops::dispatch(&op.to_string(), input.as_slice()))
    }

    #[func]
    fn load_catalog(&mut self, input: PackedByteArray) -> VarDictionary {
        envelope(|| {
            let manifest = rou_core::json::parse(input.as_slice())?;
            let catalog = viewer::Catalog::new(&manifest)?;
            let layout = viewer::catalog(&manifest)?;
            self.catalog = Some(catalog);
            Ok(serde_json::to_vec(&layout)?)
        })
    }

    #[func]
    fn catalog_query(&self, position: PackedByteArray) -> VarDictionary {
        envelope(|| {
            let result = self
                .catalog
                .as_ref()
                .ok_or_else(|| {
                    rou_core::Error::new("missing_catalog", "load a catalog before querying")
                })?
                .query(rou_core::json::parse(position.as_slice())?)?;
            Ok(serde_json::to_vec(&result)?)
        })
    }
}

fn envelope(operation: impl FnOnce() -> rou_core::Result<Vec<u8>>) -> VarDictionary {
    // Keep operation, encoding and engine-value conversion inside one boundary.
    catch_unwind(AssertUnwindSafe(|| {
        let (_, bytes) = ops::envelope(operation());
        let value: Value = serde_json::from_slice(&bytes).expect("core envelope is valid JSON");
        json_variant(&value).to::<VarDictionary>()
    }))
    .unwrap_or_else(|_| {
        let mut error = VarDictionary::new();
        error.set("code", "internal_panic");
        error.set("message", "core operation panicked");
        let mut result = VarDictionary::new();
        result.set("ok", false);
        result.set("error", &error.to_variant());
        result
    })
}

fn json_variant(value: &Value) -> Variant {
    match value {
        Value::Null => Variant::nil(),
        Value::Bool(value) => value.to_variant(),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                value.to_variant()
            } else {
                number.as_f64().expect("finite JSON number").to_variant()
            }
        }
        Value::String(value) => GString::from(value).to_variant(),
        Value::Array(values) => {
            let mut result = VarArray::new();
            for value in values {
                result.push(&json_variant(value));
            }
            result.to_variant()
        }
        Value::Object(values) => {
            let mut result = VarDictionary::new();
            for (key, value) in values {
                result.set(&GString::from(key).to_variant(), &json_variant(value));
            }
            result.to_variant()
        }
    }
}
