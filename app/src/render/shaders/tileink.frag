#version 300 es

in vec2 vP;
flat in vec2 vSA;
flat in vec2 vSB;
flat in float vHalf;
flat in float vFade;

out vec4 finalColor;

uniform float uAlpha;

const vec3 INK = vec3(0.17, 0.14, 0.11);

void main() {
    vec2 pa = vP - vSA;
    vec2 ba = vSB - vSA;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    float d = length(pa - ba * h);
    float cover = 1.0 - smoothstep(vHalf - 0.5, vHalf + 0.5, d);
    float a = cover * vFade * uAlpha;
    if (a <= 0.001) discard;
    finalColor = vec4(INK * a, a);
}
