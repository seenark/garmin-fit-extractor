const MAX_FILES = 10;
const MAX_FILE_BYTES = 20 * 1024 * 1024;
const encoder = new TextEncoder();
const controlCharacter = /[\u0000-\u001f\u007f]/;

export function validateFiles(files: readonly File[]): string[] {
  if (files.length === 0) return ["Select at least one ZIP file."];
  if (files.length > MAX_FILES) return ["Select at most 10 ZIP files."];

  const errors: string[] = [];
  for (const file of files) {
    if (!file.name.toLowerCase().endsWith(".zip")) {
      errors.push(`${file.name} must end with .zip.`);
      continue;
    }
    if (controlCharacter.test(file.name)) {
      errors.push(`${file.name} contains an invalid control character.`);
      continue;
    }
    if (encoder.encode(file.name).byteLength > 255) {
      errors.push(`${file.name} has a file name longer than 255 bytes.`);
      continue;
    }
    if (file.size > MAX_FILE_BYTES) {
      errors.push(`${file.name} is larger than the 20-megabyte upload limit.`);
    }
  }
  return errors;
}
