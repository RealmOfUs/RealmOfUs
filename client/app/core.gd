## Transport only: the extension converts every response into Godot values.
extends RefCounted
static var native: Rou
static var configuration: Dictionary = {}


static func invoke(operation: String, input: Variant = PackedByteArray()) -> Dictionary:
	var bytes: PackedByteArray = (
		input if input is PackedByteArray else JSON.stringify(input).to_utf8_buffer()
	)
	if native == null:
		native = Rou.new()
		if native.abi_version() != 1:
			native = null
			return {
				"ok": false,
				"error": {"code": "abi_mismatch", "message": "Realm extension ABI mismatch"}
			}
	return native.invoke(operation, bytes)


static func config() -> Dictionary:
	if configuration.is_empty():
		configuration = invoke("config").value
	return configuration


static func message(result: Dictionary) -> String:
	return str(result.error.message) if not result.ok else ""


static func vec(values: Array) -> Vector3:
	return Vector3(values[0], values[1], values[2])


static func array(vector: Vector3) -> Array:
	return [vector.x, vector.y, vector.z]
