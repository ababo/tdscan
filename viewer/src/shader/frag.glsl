precision mediump float;

varying float vert_element;
varying vec2 vert_texture;

uniform sampler2D textures[MAX_TEXTURE_IMAGE_UNITS];

vec4 get_texture_color(int index, vec2 point) {
    if (index == 0) return texture2D(textures[0], point);
    if (index == 1) return texture2D(textures[1], point);
    if (index == 2) return texture2D(textures[2], point);
    if (index == 3) return texture2D(textures[3], point);
    if (index == 4) return texture2D(textures[4], point);
    if (index == 5) return texture2D(textures[5], point);
    if (index == 6) return texture2D(textures[6], point);
    if (index == 7) return texture2D(textures[7], point);
#if MAX_TEXTURE_IMAGE_UNITS >= 16
    if (index == 8) return texture2D(textures[8], point);
    if (index == 9) return texture2D(textures[9], point);
    if (index == 10) return texture2D(textures[10], point);
    if (index == 11) return texture2D(textures[11], point);
    if (index == 12) return texture2D(textures[12], point);
    if (index == 13) return texture2D(textures[13], point);
    if (index == 14) return texture2D(textures[14], point);
    if (index == 15) return texture2D(textures[15], point);
#endif
#if MAX_TEXTURE_IMAGE_UNITS >= 32
    if (index == 16) return texture2D(textures[16], point);
    if (index == 17) return texture2D(textures[17], point);
    if (index == 18) return texture2D(textures[18], point);
    if (index == 19) return texture2D(textures[19], point);
    if (index == 20) return texture2D(textures[20], point);
    if (index == 21) return texture2D(textures[21], point);
    if (index == 22) return texture2D(textures[22], point);
    if (index == 23) return texture2D(textures[23], point);
    if (index == 24) return texture2D(textures[24], point);
    if (index == 25) return texture2D(textures[25], point);
    if (index == 26) return texture2D(textures[26], point);
    if (index == 27) return texture2D(textures[27], point);
    if (index == 28) return texture2D(textures[28], point);
    if (index == 29) return texture2D(textures[29], point);
    if (index == 30) return texture2D(textures[30], point);
    if (index == 31) return texture2D(textures[31], point);
#endif
    return vec4(0.0, 0.0, 0.0, 0.0);
}

void main() {
    gl_FragColor = get_texture_color(int(vert_element), vert_texture);
}
