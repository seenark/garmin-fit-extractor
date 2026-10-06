import { useId, useRef, useState } from "react";

const chatGptTemplate = `ช่วยอ่านข้อมูลกิจกรรมวิ่งที่ฉันแนบ แล้วสรุปสิ่งที่ข้อมูลรองรับจริง

เป้าหมายของฉัน: [แก้ไขเป้าหมายที่นี่]
บริบทการซ้อมและความรู้สึกระหว่างวิ่ง: [เพิ่มบริบทที่นี่]

แยกค่าที่อุปกรณ์บันทึก ผลวิเคราะห์ที่คำนวณ และข้อสันนิษฐานออกจากกัน ตรวจคุณภาพข้อมูล ช่วงวิ่ง เวลา และ laps ก่อนแนะนำการซ้อม
หากข้อมูลไม่พอ ให้บอกว่าขาดอะไร ห้ามสร้างค่าที่ไม่มี ห้ามใช้ชีพจรเฉลี่ยแทน RR และอย่าอ้างว่า LT เป็นผลตรวจ blood lactate
แยก LT1 และ LT2 พร้อมวิธี หลักฐาน ความไม่แน่นอน และ evidence cutoff ของแต่ละค่า
ตอบเป็นภาษาที่เข้าใจง่าย พร้อมคำถามที่จำเป็นก่อนเสนอแผนซ้อม`;

const claudeTemplate = `อ่านข้อมูลกิจกรรมวิ่งที่ฉันแนบในฐานะผู้ช่วยโค้ชที่ตรวจสอบหลักฐาน

คำถามที่อยากให้ช่วยตอบ: [แก้ไขคำถามที่นี่]
เป้าหมายและข้อจำกัดในการซ้อม: [เพิ่มบริบทที่นี่]

เริ่มจากตรวจคุณภาพและข้อจำกัดของข้อมูล จากนั้นเปรียบเทียบ summary, laps และช่วงที่ระบบตรวจพบโดยไม่สมมติเจตนาของการวิ่ง
อ้างอิงค่าหรือช่วงเวลาที่รองรับข้อสรุป แยกข้อมูล recorded, device-reported และ derived ออกจากกัน ไม่สร้างค่าที่ไม่มีหรือข้อมูลกิจกรรมที่ไม่ได้แนบ
ประเมิน LT1 และ LT2 แยกกัน คง uncertainty และ evidence cutoff และยอมรับ insufficient_data เมื่อหลักฐานไม่พอ ผลเหล่านี้ไม่ใช่การวัด blood lactate
ปิดท้ายด้วยสิ่งที่ควรตรวจสอบเพิ่มและคำแนะนำการซ้อมที่สอดคล้องกับหลักฐาน ไม่ใช่การวินิจฉัยทางการแพทย์`;

function PromptTemplate({ name, initialText }: { name: string; initialText: string }) {
  const id = useId();
  const field = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState(initialText);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<"idle" | "copied" | "failed">("idle");
  async function copy() {
    setBusy(true);
    setStatus("idle");
    try {
      await navigator.clipboard.writeText(text);
      setStatus("copied");
    } catch {
      setStatus("failed");
      field.current?.focus();
      field.current?.select();
    } finally { setBusy(false); }
  }
  return <div className="coach-prompt-template">
    <label htmlFor={id}>{name} prompt</label>
    <textarea id={id} ref={field} rows={12} value={text} readOnly={busy} aria-describedby={`${id}-status`} onChange={(event) => { setText(event.target.value); setStatus("idle"); }} />
    <button type="button" className="secondary" disabled={busy} aria-busy={busy} data-state={status === "copied" ? "success" : status === "failed" ? "error" : undefined} onClick={copy}>
      {busy ? "กำลังคัดลอก…" : `Copy ${name} prompt`}
    </button>
    <p id={`${id}-status`} role={status === "failed" ? "alert" : "status"} className={status === "failed" ? "error-text" : "coach-prompt-status"}>
      {status === "copied" ? "คัดลอกข้อความปัจจุบันในช่องแล้ว" : status === "failed" ? "คัดลอกไม่สำเร็จ เลือกข้อความในช่องแล้วกดคัดลอกด้วยตนเอง" : "แก้ไขข้อความได้ คัดลอกเฉพาะข้อความปัจจุบันในช่องนี้"}
    </p>
  </div>;
}

export function CoachPromptEditor() {
  return <section className="coach-prompt-editor" aria-label="แก้ไข prompt สำหรับโค้ช">
    <h2>ข้อความสำหรับคุยกับโค้ช</h2>
    <p>สอง template แก้ไขแยกกัน คัดลอก prompt แล้วแนบไฟล์ JSON ที่ส่งออกเอง หน้านี้ไม่ส่งข้อมูลให้ ChatGPT หรือ Claude และไม่ขอ API key</p>
    <div className="coach-prompt-templates">
      <PromptTemplate name="ChatGPT" initialText={chatGptTemplate} />
      <PromptTemplate name="Claude" initialText={claudeTemplate} />
    </div>
  </section>;
}
