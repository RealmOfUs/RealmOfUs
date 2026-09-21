shader_type spatial;
render_mode cull_disabled;
varying vec3 world_pos;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	// Small ripples break up the reflected building without painted caustic stripes.
	float a = world_pos.x * 2.2 + world_pos.z * 1.4 + TIME * 0.42;
	float b = world_pos.x * 4.7 - world_pos.z * 3.1 - TIME * 0.55;
	float c = world_pos.x * 9.3 + world_pos.z * 6.8 + TIME * 0.71;
	vec2 slope = vec2(
		cos(a) * 0.008 + cos(b) * 0.005 + cos(c) * 0.002,
		sin(a) * 0.008 - sin(b) * 0.005 + sin(c) * 0.002
	);
	ALBEDO = vec3(0.025, 0.14, 0.145);
	ROUGHNESS = 0.12;
	METALLIC = 0.05;
	SPECULAR = 0.8;
	NORMAL_MAP = vec3(0.5 + slope.x, 0.5 + slope.y, 1.0);
}
