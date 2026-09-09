import { useEffect, useState } from "react";

type ShoeImageProps = {
  alt: string;
  imageUrl?: string;
  priority?: boolean;
  compact?: boolean;
};

/** Keeps catalog cards useful when an asset is absent or fails to load. */
export function ShoeImage({ alt, imageUrl, priority = false, compact = false }: ShoeImageProps) {
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [imageUrl]);

  const showFallback = !imageUrl || failed;
  return (
    <div className={`shoe-image-frame${compact ? " shoe-image-frame--compact" : ""}`}>
      {showFallback ? (
        <div className="shoe-image-fallback" role="img" aria-label={`${alt} — ภาพสินค้าไม่พร้อม`}>
          <span className="shoe-image-fallback-mark" aria-hidden="true">
            SHOE
          </span>
          <span>ภาพสินค้าไม่พร้อม</span>
        </div>
      ) : (
        <img
          alt={alt}
          className="shoe-image"
          decoding="async"
          fetchPriority={priority ? "high" : "auto"}
          height={480}
          loading={priority ? "eager" : "lazy"}
          onError={() => setFailed(true)}
          src={imageUrl}
          width={640}
        />
      )}
    </div>
  );
}
