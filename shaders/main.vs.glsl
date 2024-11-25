#version 450
#extension GL_EXT_buffer_reference: require
#extension GL_EXT_scalar_block_layout: require

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


layout(buffer_reference, buffer_reference_align = 4, scalar) readonly buffer BufferPtr {
	float time;
};


layout(push_constant, std430) uniform constants {
	BufferPtr u_buffer;
};

layout(location = 0) out vec3 v_color;

void main() {
	gl_Position = vec4(c_positions[gl_VertexIndex % 3] + vec2(cos(u_buffer.time), sin(u_buffer.time)) * 0.2, 0, 1);
	v_color = c_colors[gl_VertexIndex % 3];
}
