import { Link } from "@tanstack/react-router";

import { formatReleaseDate, type Shoe } from "../../domain/catalog";
import { ShoeImage } from "./ShoeImage";

type ShoeCardProps = {
  shoe: Shoe;
  reviewCount: number;
};

export function ShoeCard({ shoe, reviewCount }: ShoeCardProps) {
  const label = `${shoe.brand} ${shoe.model}`;
  return (
    <article className="shoe-card">
      <Link
        aria-label={`ดูรายละเอียด ${label}`}
        className="shoe-card-link"
        params={{ shoeId: shoe.id }}
        to="/shoes/$shoeId"
      >
        <ShoeImage alt={label} compact imageUrl={shoe.imageUrl} />
        <div className="shoe-card-body">
          <div className="shoe-card-heading">
            <p className="shoe-card-brand">{shoe.brand}</p>
            <span className="shoe-card-arrow" aria-hidden="true">
              ↗
            </span>
          </div>
          <h3 className="shoe-card-title">{shoe.model}</h3>
          <p className="shoe-card-release">
            เริ่มขาย <time dateTime={`${shoe.releaseDate.year}-${String(shoe.releaseDate.month).padStart(2, "0")}`}>
              {formatReleaseDate(shoe.releaseDate)}
            </time>
          </p>
          <div className="shoe-card-footer">
            <span className="status-badge">
              <span className="status-dot" aria-hidden="true" />
              {reviewCount > 0 ? `${reviewCount} รีวิว` : "ยังไม่มีรีวิว"}
            </span>
            <span className="shoe-card-cta" aria-hidden="true">
              ดูรายละเอียด
            </span>
          </div>
        </div>
      </Link>
    </article>
  );
}
