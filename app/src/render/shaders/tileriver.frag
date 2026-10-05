#version 300 es
// Antialiased capsule around a river segment, one flat color (no seams where capsules overlap).

in vec2 vP;
flat in vec2 vSA;
flat in vec2 vSB;
in float vHalf;
in float vFade;

out vec4 finalColor;

uniform float uAlpha;

const vec3 WATER = vec3(0.41, 0.53, 0.61);
const vec3 INK = vec3(0.23, 0.33, 0.42);

void main() {
    vec2 pa = vP - vSA;
    vec2 ba = vSB - vSA;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    float d = length(pa - ba * h);
    float cover = 1.0 - smoothstep(vHalf - 0.5, vHalf + 0.5, d);
    vec3 col = vHalf > 1.5 ? WATER : INK;
    float a = cover * vFade * uAlpha;
    if (a <= 0.001) discard;
    finalColor = vec4(col * a, a);
}
