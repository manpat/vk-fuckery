#version 450

const vec2[] c_positions = vec2[](
	vec2( 0.0,-0.5),
	vec2(-0.5, 0.7),
	vec2( 0.5, 0.7)
);

const vec3[] c_colors = vec3[](
	vec3(1.0, 0.0, 0.0),
	vec3(0.0, 1.0, 0.0),
	vec3(0.0, 0.0, 1.0)
);


layout(push_constant) uniform constants {
	float u_time;
};

layout(location = 0) out vec3 v_color;

void main() {
	gl_Position = vec4(c_positions[gl_VertexIndex % 3] + vec2(cos(u_time), sin(u_time)) * 0.2, 0, 1);
	v_color = c_colors[gl_VertexIndex % 3];
}