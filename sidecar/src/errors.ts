export class ConvertZZError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly details?: unknown,
  ) {
    super(message);
    this.name = "ConvertZZError";
  }
}

export function toErrorPayload(error: unknown): {
  code: string;
  message: string;
  details?: unknown;
} {
  if (error instanceof ConvertZZError) {
    return { code: error.code, message: error.message, details: error.details };
  }

  if (error instanceof Error) {
    return { code: "UNEXPECTED_ERROR", message: error.message };
  }

  return { code: "UNEXPECTED_ERROR", message: String(error) };
}
