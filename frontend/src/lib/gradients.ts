// Placeholder property-image gradients, ported from the prototype. Swap for real
// imagery later; deterministic by index so cards stay stable.
export const GRADS = [
  "linear-gradient(150deg,#E9764D,#C5392B 60%,#7c2a1f)",
  "linear-gradient(150deg,#3a6ea5,#274472)",
  "linear-gradient(150deg,#6a8d5b,#3d5a36)",
  "linear-gradient(150deg,#caa15a,#9c6f2e)",
  "linear-gradient(150deg,#7d6a9c,#4b3a6b)",
  "linear-gradient(150deg,#b5563f,#7a3322)",
];

export function gradFor(i: number) {
  return GRADS[i % GRADS.length];
}
