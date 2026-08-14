declare module "encoding-japanese" {
  interface ConvertOptions {
    to: string;
    from?: string;
    type?: "string" | "array";
  }

  const Encoding: {
    convert(input: number[] | Uint8Array | string, options: ConvertOptions): string | number[];
    detect(input: number[] | Uint8Array, encoding?: string): string | false;
  };
  export default Encoding;
}
