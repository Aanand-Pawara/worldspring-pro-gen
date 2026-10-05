#version 300 es
// Pre-tessellated, vertex-coloured triangles (battlemap roofs), in chunk pixels.

in vec2 aPos;
in vec4 aColor;

out vec4 vColor;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vColor = aColor;
    gl_Position = vec4((uProjectionMatrix * m * vec3(aPos, 1.0)).xy, 0.0, 1.0);
}
