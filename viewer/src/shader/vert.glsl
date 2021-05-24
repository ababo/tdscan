#version 300 es

precision mediump float;

in vec2 texture;
in vec3 position;

out vec2 vert_texture;
flat out int vert_vertex_id;

uniform mat4 projection;
uniform mat4 view;

void main() {
    vert_texture = texture;
    vert_vertex_id = gl_VertexID;
    gl_Position = projection * view * vec4(position, 1.0);
}
