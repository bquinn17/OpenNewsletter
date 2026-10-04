const CROCKFORD_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const TIME_LENGTH = 10;
const RANDOM_LENGTH = 16;

function encodeTime(timestampMs: number, length: number): string {
  let remaining = timestampMs;
  let encoded = "";
  for (let i = 0; i < length; i++) {
    const digit = remaining % CROCKFORD_ALPHABET.length;
    encoded = CROCKFORD_ALPHABET[digit] + encoded;
    remaining = (remaining - digit) / CROCKFORD_ALPHABET.length;
  }
  return encoded;
}

function encodeRandom(length: number): string {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  let encoded = "";
  for (let i = 0; i < length; i++) {
    encoded += CROCKFORD_ALPHABET[bytes[i] % CROCKFORD_ALPHABET.length];
  }
  return encoded;
}

/** A ULID (26-char Crockford base32, time-prefixed) suitable as an `x-correlation-id`. */
export function newCorrelationId(): string {
  return encodeTime(Date.now(), TIME_LENGTH) + encodeRandom(RANDOM_LENGTH);
}
