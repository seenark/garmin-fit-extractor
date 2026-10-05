const MAX_FILES = 10;
const MAX_FILE_BYTES = 20 * 1024 * 1024;
const encoder = new TextEncoder();
const controlCharacter = /[\u0000-\u001f\u007f]/;

export function validateFiles(files: readonly File[]): string[] {
  if (files.length === 0) return ["เลือกไฟล์ FIT หรือ ZIP อย่างน้อย 1 ไฟล์"];
  if (files.length > MAX_FILES) return ["เลือกได้ไม่เกิน 10 ไฟล์"];

  const errors: string[] = [];
  for (const file of files) {
    if (!/\.(fit|zip)$/i.test(file.name)) {
      errors.push(`${file.name} ต้องลงท้ายด้วย .fit หรือ .zip`);
      continue;
    }
    if (controlCharacter.test(file.name)) {
      errors.push(`${file.name} มีอักขระควบคุมที่ใช้ไม่ได้`);
      continue;
    }
    if (encoder.encode(file.name).byteLength > 255) {
      errors.push(`${file.name} ยาวเกิน 255 ไบต์`);
      continue;
    }
    if (file.size === 0) {
      errors.push(`${file.name} เป็นไฟล์ว่าง`);
      continue;
    }
    if (file.size > MAX_FILE_BYTES) {
      errors.push(`${file.name} มีขนาดเกิน 20 MiB`);
    }
  }
  return errors;
}
