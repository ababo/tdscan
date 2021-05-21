precision mediump float;

attribute vec2 texture;
attribute vec3 position;

uniform mat4 projection;
uniform mat4 view;
uniform mat4 world;

varying vec2 frag_texture;

void main() {
    frag_texture = texture;
    gl_Position = projection * view * world * vec4(position, 1.0);
}
