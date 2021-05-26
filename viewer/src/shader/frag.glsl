#version 300 es

precision mediump float;

in vec2 vert_texture;
flat in int vert_vertex_id;

out vec4 frag_color;

uniform int texture_index[MAX_TEXTURE_IMAGE_UNITS];
uniform sampler2D textures[MAX_TEXTURE_IMAGE_UNITS];

vec4 get_texture_color(int index, vec2 point) {
    switch (index) {
        case 0: return texture(textures[0], point);
        case 1: return texture(textures[1], point);
        case 2: return texture(textures[2], point);
        case 3: return texture(textures[3], point);
        case 4: return texture(textures[4], point);
        case 5: return texture(textures[5], point);
        case 6: return texture(textures[6], point);
        case 7: return texture(textures[7], point);
#if MAX_TEXTURE_IMAGE_UNITS >= 16
        case 8: return texture(textures[8], point);
        case 9: return texture(textures[9], point);
        case 10: return texture(textures[10], point);
        case 11: return texture(textures[11], point);
        case 12: return texture(textures[12], point);
        case 13: return texture(textures[13], point);
        case 14: return texture(textures[14], point);
        case 15: return texture(textures[15], point);
#endif
#if MAX_TEXTURE_IMAGE_UNITS >= 32
        case 16: return texture(textures[16], point);
        case 17: return texture(textures[17], point);
        case 18: return texture(textures[18], point);
        case 19: return texture(textures[19], point);
        case 20: return texture(textures[20], point);
        case 21: return texture(textures[21], point);
        case 22: return texture(textures[22], point);
        case 23: return texture(textures[23], point);
        case 24: return texture(textures[24], point);
        case 25: return texture(textures[25], point);
        case 26: return texture(textures[26], point);
        case 27: return texture(textures[27], point);
        case 28: return texture(textures[28], point);
        case 29: return texture(textures[29], point);
        case 30: return texture(textures[30], point);
        case 31: return texture(textures[31], point);
#endif
    }
}

void main() {
    for (int i = 0; i < MAX_TEXTURE_IMAGE_UNITS; ++i) {
        if (vert_vertex_id < texture_index[i]) {
            frag_color = get_texture_color(i, vert_texture);
            break;
        }
    }
}
