#version 300 es

precision mediump float;

in vec2 vert_texture;
flat in int vert_vertex_id;

out vec4 frag_color;

uniform int texture_index[MAX_TEXTURE_IMAGE_UNITS];
uniform sampler2D textures[MAX_TEXTURE_IMAGE_UNITS];

#define TRY_TEXTURE(index) \
    if (vert_vertex_id < texture_index[index]) { \
        frag_color = texture(textures[index], vert_texture); \
    } else

void main() {
    TRY_TEXTURE(0)
    TRY_TEXTURE(1)
    TRY_TEXTURE(2)
    TRY_TEXTURE(3)
    TRY_TEXTURE(4)
    TRY_TEXTURE(5)
    TRY_TEXTURE(6)
    TRY_TEXTURE(7)
#if MAX_TEXTURE_IMAGE_UNITS >= 16
    TRY_TEXTURE(8)
    TRY_TEXTURE(9)
    TRY_TEXTURE(10)
    TRY_TEXTURE(11)
    TRY_TEXTURE(12)
    TRY_TEXTURE(13)
    TRY_TEXTURE(14)
    TRY_TEXTURE(15)
#endif
#if MAX_TEXTURE_IMAGE_UNITS >= 32
    TRY_TEXTURE(16)
    TRY_TEXTURE(17)
    TRY_TEXTURE(18)
    TRY_TEXTURE(19)
    TRY_TEXTURE(20)
    TRY_TEXTURE(21)
    TRY_TEXTURE(22)
    TRY_TEXTURE(23)
    TRY_TEXTURE(24)
    TRY_TEXTURE(25)
    TRY_TEXTURE(26)
    TRY_TEXTURE(27)
    TRY_TEXTURE(28)
    TRY_TEXTURE(29)
    TRY_TEXTURE(30)
    TRY_TEXTURE(31)
#endif
    {}
}
