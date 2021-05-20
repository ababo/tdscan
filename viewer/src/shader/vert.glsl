precision mediump float;

attribute vec2 texture;
attribute vec3 position;

void main() {
    // This nonsense is just to make sure the attributes are not optimized out.
    gl_Position = vec4(vec3(texture, 0.0) + position, 1.0);
}
