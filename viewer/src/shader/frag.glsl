precision mediump float;

uniform sampler2D textures[32];

varying vec2 frag_texture;

void main() {
    gl_FragColor = texture2D(textures[0], frag_texture);
}
