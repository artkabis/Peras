// A blur of an image (decision 0031, program P3), on one thread: each pixel
// of the output is the mean of the 3 × 3 pixels around it in the input,
// the edges repeated. The reference the other versions are checked against.
export function blur(
  input: Float64Array,
  output: Float64Array,
  width: number,
  height: number,
): number {
  for (let y = 0; y < height; y++) {
    const up = y === 0 ? 0 : y - 1;
    const down = y === height - 1 ? height - 1 : y + 1;
    for (let x = 0; x < width; x++) {
      const left = x === 0 ? 0 : x - 1;
      const right = x === width - 1 ? width - 1 : x + 1;
      let sum = 0;
      for (const row of [up, y, down]) {
        sum += input[row * width + left]! + input[row * width + x]! + input[row * width + right]!;
      }
      output[y * width + x] = sum / 9;
    }
  }
  return width * height;
}
