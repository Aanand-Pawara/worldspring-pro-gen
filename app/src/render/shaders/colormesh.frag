#version 300 es
// Straight-alpha vertex colour, premultiplied, times the container's tint and alpha.

in vec4 vColor;

out vec4 finalColor;

uniform vec4 uColor;

void main() {
    finalColor = vec4(vColor.rgb * vColor.a, vColor.a) * uColor;
}
