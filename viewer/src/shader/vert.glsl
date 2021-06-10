precision mediump float;

attribute float element;
attribute vec2 texture;
attribute vec3 vertex;

varying float vert_element;
varying vec2 vert_texture;

uniform mat4 projection;
uniform mat4 view;

void main() {
    vert_element = element;
    vert_texture = texture;
    gl_Position = projection * view * vec4(vertex, 1.0);
}
