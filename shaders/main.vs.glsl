#version 450

const vec2[] c_positions = vec2[](
	vec2( 0.0,-0.5),
	vec2(-0.5, 0.7),
	vec2( 0.5, 0.7)
);

void main() {
	gl_Position = vec4(c_positions[gl_VertexIndex % 3], 0, 1);
}