import type { ApiErrorDetail } from "./api-types";

const apiErrorMessages: Record<string, string> = {
  REQUEST_FAILED: "ระบบไม่สามารถดำเนินการตามคำขอได้ ลองใหม่อีกครั้ง",
  INVALID_MULTIPART: "ไฟล์ที่ส่งมาอ่านไม่ได้ ลองเลือกไฟล์ ZIP ใหม่อีกครั้ง",
  EMPTY_BATCH: "เลือกไฟล์ ZIP อย่างน้อย 1 ไฟล์ก่อนอัปโหลด",
  UNKNOWN_FIELD: "คำขอนี้ไม่ถูกต้อง ระบบรับเฉพาะช่อง files",
  TOO_MANY_FILES: "อัปโหลดได้ไม่เกิน 10 ไฟล์ต่อครั้ง",
  REQUEST_TOO_LARGE: "ไฟล์ที่ส่งมามีขนาดใหญ่เกินไป",
  INVALID_FILE_NAME: "ชื่อไฟล์ต้องลงท้ายด้วย .zip และยาวไม่เกิน 255 ไบต์",
  FILE_TOO_LARGE: "ไฟล์ ZIP หรือ FIT มีขนาดเกิน 20 เมกะไบต์",
  INVALID_ZIP: "ไฟล์นี้ไม่ใช่ ZIP ที่ถูกต้อง หรือไฟล์เสียหาย",
  ARCHIVE_LIMIT_EXCEEDED: "ไฟล์ ZIP มีข้อมูล FIT มากเกินกว่าที่ระบบรองรับ",
  INVALID_FIT: "ไฟล์นี้ไม่ใช่ FIT ที่ถูกต้อง หรือไฟล์เสียหาย",
  INVALID_PAGINATION: "ค่าหน้าและลำดับที่ส่งมาไม่ถูกต้อง",
  INVALID_ID: "รหัสรายการไม่ถูกต้อง",
  NOT_FOUND: "ไม่พบรายการที่ต้องการ",
  INVALID_VIEW: "มุมมองข้อมูลต้องเป็น normalized หรือ raw",
  INVALID_ACTIVITY_LIMIT: "จำนวนรายการต้องอยู่ระหว่าง 1 ถึง 20",
  EXTRACTION_FAILED: "รายการที่แยกข้อมูลไม่สำเร็จยังไม่มี JSON ให้ดาวน์โหลด",
  PROCESSING_ERROR: "ระบบประมวลผลไฟล์ FIT ไม่สำเร็จ",
  DATABASE_ERROR: "ระบบบันทึกรายการไม่สำเร็จ",
  SERVICE_UNAVAILABLE: "ระบบไม่พร้อมใช้งานชั่วคราว ลองใหม่อีกครั้ง",
  AUTH_REQUIRED: "กรุณาเข้าสู่ระบบด้วย Google ก่อน",
  AUTH_NOT_CONFIGURED: "ยังไม่ได้ตั้งค่าการเข้าสู่ระบบด้วย Google",
  AUTH_PROVIDER_UNAVAILABLE: "การเข้าสู่ระบบด้วย Google ใช้งานไม่ได้ชั่วคราว",
  COACH_NOT_CONFIGURED: "ยังไม่ได้ตั้งค่า FIT Coach OAuth",
  COACH_AUTHENTICATION_FAILED: "การยืนยันตัวตนของ FIT Coach ไม่สำเร็จ",
  INSUFFICIENT_SCOPE: "โทเค็นนี้ไม่มีสิทธิ์อ่านข้อมูลกิจกรรม",
  ACTIVITY_NOT_FOUND: "ไม่พบกิจกรรมที่ต้องการ",
  INVALID_ACTIVITY_DETAIL: "รายละเอียดกิจกรรมที่ขอไม่ถูกต้อง",
  COACH_PROCESSING_ERROR: "ระบบประมวลผลกิจกรรมไม่สำเร็จ",
  NO_FIT_FILES: "ใน ZIP นี้ไม่พบไฟล์ FIT",
};

export function formatApiError(
  detail: Pick<ApiErrorDetail, "code" | "message"> | null | undefined,
): string {
  if (!detail) return "เกิดข้อผิดพลาด ลองใหม่อีกครั้ง";
  return apiErrorMessages[detail.code] ?? "ระบบทำรายการนี้ไม่สำเร็จ ลองใหม่อีกครั้ง";
}
