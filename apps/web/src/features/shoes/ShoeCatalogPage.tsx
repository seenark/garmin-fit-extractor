import { useMemo, useState } from "react";

import {
  filterShoesByReleaseDate,
  formatReleaseMonth,
  getAvailableReleaseYears,
  sortShoes,
  type Catalog,
  type ShoeSortOption,
} from "../../domain/catalog";
import { ShoeCard } from "./ShoeCard";

type ShoeCatalogPageProps = {
  catalog: Catalog;
};

const months = Array.from({ length: 12 }, (_, index) => index + 1);

export function ShoeCatalogPage({ catalog }: ShoeCatalogPageProps) {
  const [query, setQuery] = useState("");
  const [selectedYear, setSelectedYear] = useState("all");
  const [selectedMonth, setSelectedMonth] = useState("all");
  const [sortOption, setSortOption] = useState<ShoeSortOption>("latest");
  const availableYears = useMemo(() => getAvailableReleaseYears(catalog.shoes), [catalog.shoes]);

  const filteredShoes = useMemo(() => {
    const year = selectedYear === "all" ? null : Number(selectedYear);
    const month = selectedMonth === "all" ? null : Number(selectedMonth);
    return sortShoes(
      filterShoesByReleaseDate(catalog.shoes, { query, year, month }),
      sortOption,
    );
  }, [catalog.shoes, query, selectedMonth, selectedYear, sortOption]);

  return (
    <section className="shoe-catalog-page" aria-labelledby="shoe-catalog-title">
      <header className="shoe-catalog-hero">
        <p className="shoe-eyebrow">RUNNING SHOE LIBRARY</p>
        <h1 id="shoe-catalog-title">ค้นหารองเท้าวิ่งจากข้อมูลคนใส่จริง</h1>
        <p className="shoe-catalog-lede">
          รวมคำแนะนำเรื่องไซซ์จากผู้รีวิว พร้อมข้อมูลตารางไซซ์และแหล่งที่มาที่ตรวจสอบต่อได้
        </p>
      </header>

      <section className="shoe-reviewer-note" aria-labelledby="shoe-reviewer-note-title">
        <div>
          <p className="shoe-eyebrow">FIELD NOTES</p>
          <h2 id="shoe-reviewer-note-title">ข้อมูลจากการใช้งานจริง</h2>
        </div>
        <div className="shoe-reviewer-note-copy">
          <p>
            ข้อมูลไซซ์บนเว็บนี้มาจากประสบการณ์จริงและคำแนะนำของผู้รีวิวหลายช่อง เรารวบรวมให้ค้นหา
            และเปรียบเทียบง่ายขึ้น
          </p>
          <p>
            เปิดแหล่งข้อมูลต้นทางจากหน้ารุ่นได้เสมอ เพราะฟิต ความรู้สึก และการใช้งานจริงอาจแตกต่าง
            จากคำแนะนำสั้น ๆ
          </p>
        </div>
      </section>

      <section className="shoe-catalog-list" aria-labelledby="shoe-catalog-list-title">
        <div className="shoe-section-heading">
          <div>
            <p className="shoe-eyebrow">CATALOG INDEX</p>
            <h2 id="shoe-catalog-list-title">รุ่นที่มีข้อมูล</h2>
            <p>เลือกหนึ่งรุ่นเพื่อดูสรุปรีวิว ตารางไซซ์ และเครื่องมือเทียบไซซ์</p>
          </div>
          <p className="shoe-result-count" role="status" aria-live="polite">
            <strong>{filteredShoes.length}</strong> รุ่น
          </p>
        </div>

        <form className="shoe-filter-bar" role="search" onSubmit={(event) => event.preventDefault()}>
          <label className="shoe-field shoe-field--search" htmlFor="shoe-search">
            <span>ค้นหาแบรนด์หรือรุ่นรองเท้า</span>
            <input
              id="shoe-search"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="เช่น PUMA Deviate Pure NITRO"
              type="search"
              value={query}
            />
          </label>
          <label className="shoe-field" htmlFor="release-year-filter">
            <span>ปีที่เริ่มขาย</span>
            <select
              id="release-year-filter"
              onChange={(event) => setSelectedYear(event.target.value)}
              value={selectedYear}
            >
              <option value="all">ทุกปี</option>
              {availableYears.map((year) => (
                <option key={year} value={year}>
                  {year}
                </option>
              ))}
            </select>
          </label>
          <label className="shoe-field" htmlFor="release-month-filter">
            <span>เดือนที่เริ่มขาย</span>
            <select
              id="release-month-filter"
              onChange={(event) => setSelectedMonth(event.target.value)}
              value={selectedMonth}
            >
              <option value="all">ทุกเดือน</option>
              {months.map((month) => (
                <option key={month} value={month}>
                  {formatReleaseMonth(month)}
                </option>
              ))}
            </select>
          </label>
          <label className="shoe-field" htmlFor="shoe-sort">
            <span>เรียงลำดับ</span>
            <select
              id="shoe-sort"
              onChange={(event) => setSortOption(event.target.value as ShoeSortOption)}
              value={sortOption}
            >
              <option value="latest">วางขายล่าสุดก่อน</option>
              <option value="oldest">วางขายเก่าสุดก่อน</option>
              <option value="name-asc">ชื่อ A-Z</option>
              <option value="name-desc">ชื่อ Z-A</option>
              <option value="year-desc">ปีใหม่ไปเก่า</option>
              <option value="year-asc">ปีเก่าไปใหม่</option>
              <option value="month-desc">เดือนหลังไปต้นปี</option>
              <option value="month-asc">เดือนต้นปีไปปลายปี</option>
            </select>
          </label>
        </form>

        {filteredShoes.length === 0 ? (
          <div className="empty-state shoe-empty-state">
            <span className="empty-state-mark" aria-hidden="true">?</span>
            <h2>ยังไม่เจอรุ่นที่ค้นหา</h2>
            <p>ลองพิมพ์แค่ชื่อแบรนด์ ใช้ชื่อรุ่นให้สั้นลง หรือล้างตัวกรองปีและเดือน</p>
            <button
              className="secondary"
              type="button"
              onClick={() => {
                setQuery("");
                setSelectedYear("all");
                setSelectedMonth("all");
              }}
            >
              ล้างตัวกรอง
            </button>
          </div>
        ) : (
          <div className="shoe-card-grid">
            {filteredShoes.map((shoe) => (
              <ShoeCard
                key={shoe.id}
                shoe={shoe}
                reviewCount={catalog.reviews.filter((review) => review.shoeId === shoe.id).length}
              />
            ))}
          </div>
        )}
      </section>
    </section>
  );
}
