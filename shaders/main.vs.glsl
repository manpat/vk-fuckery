#version 450
#extension GL_EXT_buffer_reference: require
#extension GL_EXT_scalar_block_layout: require

const vec2[] c_positions = vec2[](
	vec2( 0.0,-1.0),
	vec2(-1.0, 1.0),
	vec2( 1.0, 1.0)
);

const vec3[] c_colors = vec3[](
	vec3(1.0, 0.0, 0.0),
	vec3(0.0, 1.0, 0.0),
	vec3(0.0, 0.0, 1.0)
);


layout(buffer_reference, buffer_reference_align = 8, scalar) readonly buffer GlobalBufferPtr {
	mat4 projection_view;
	float time;
};

layout(buffer_reference, buffer_reference_align = 8, scalar) readonly buffer PerDrawBufferPtr {
	vec3 offset;
	float time_offset;
};


layout(push_constant, std430) uniform constants {
	GlobalBufferPtr u_global;
	PerDrawBufferPtr u_per_draw;
};

layout(location = 0) out vec3 v_color;

void main() {
	float time = u_global.time + u_per_draw.time_offset;

	vec3 offset = vec3(cos(time) * 0.2, sin(time) * 0.2, (sin(time / 4.0) + 1.0) * 3.0);
	vec3 vertex = vec3(c_positions[gl_VertexIndex % 3], 0.0);
	gl_Position = u_global.projection_view * vec4(vertex + offset + u_per_draw.offset, 1);
	v_color = c_colors[gl_VertexIndex % 3];
}
