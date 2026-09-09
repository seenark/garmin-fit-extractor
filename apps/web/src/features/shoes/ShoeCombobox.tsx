import { useEffect, useMemo, useRef, useState } from "react";

import {
  filterShoesByQuery,
  groupShoesByBrand,
  type Shoe,
} from "../../domain/catalog";

type ShoeComboboxProps = {
  id: string;
  label: string;
  options: readonly Shoe[];
  value: string;
  onChange: (shoeId: string) => void;
};

export function ShoeCombobox({
  id,
  label,
  options,
  value,
  onChange,
}: ShoeComboboxProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState("");
  const selectedShoe = options.find((shoe) => shoe.id === value);
  const selectedLabel = selectedShoe
    ? `${selectedShoe.brand} ${selectedShoe.model}`
    : "";
  const groups = useMemo(
    () => groupShoesByBrand(filterShoesByQuery(options, query)),
    [options, query],
  );
  const optionCount = groups.reduce((count, group) => count + group.shoes.length, 0);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setIsOpen(false);
        setQuery("");
      }
    }

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, []);

  function open() {
    setIsOpen(true);
    setQuery("");
  }

  function selectShoe(shoeId: string) {
    onChange(shoeId);
    setIsOpen(false);
    setQuery("");
    inputRef.current?.focus();
  }

  return (
    <div className="shoe-field shoe-combobox-field">
      <span>{label}</span>
      <div className={`shoe-combobox${isOpen ? " is-open" : ""}`} ref={rootRef}>
        <div className="shoe-combobox-control">
          <input
            ref={inputRef}
            id={id}
            aria-autocomplete="list"
            aria-controls={`${id}-options`}
            aria-expanded={isOpen}
            aria-haspopup="listbox"
            aria-label={label}
            autoComplete="off"
            onChange={(event) => {
              setQuery(event.target.value);
              setIsOpen(true);
            }}
            onClick={() => {
              if (!isOpen) open();
            }}
            onFocus={open}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                setIsOpen(false);
                setQuery("");
                return;
              }

              if (event.key === "ArrowDown") {
                event.preventDefault();
                if (!isOpen) {
                  open();
                  return;
                }
                document
                  .getElementById(`${id}-options`)
                  ?.querySelector<HTMLElement>("[data-combobox-option]")
                  ?.focus();
                return;
              }

              if (event.key === "Enter" && isOpen && optionCount === 1) {
                event.preventDefault();
                const onlyOption = groups[0]?.shoes[0];
                if (onlyOption) selectShoe(onlyOption.id);
              }
            }}
            placeholder={
              isOpen
                ? "พิมพ์ชื่อแบรนด์หรือรุ่น"
                : selectedLabel
                  ? `เลือกแล้ว: ${selectedLabel}`
                  : "พิมพ์ชื่อแบรนด์หรือรุ่น"
            }
            role="combobox"
            value={isOpen ? query : selectedLabel}
          />
          <button
            aria-label={isOpen ? "ปิดตัวเลือกชุดรองเท้า" : "เปิดตัวเลือกชุดรองเท้า"}
            className="shoe-combobox-toggle"
            onClick={() => (isOpen ? setIsOpen(false) : open())}
            type="button"
          >
            <span aria-hidden="true">{isOpen ? "×" : "⌄"}</span>
          </button>
        </div>

        {isOpen ? (
          <div
            className="shoe-combobox-list"
            id={`${id}-options`}
            role="listbox"
            aria-label={`${label} — ผลการค้นหา`}
          >
            {groups.length > 0 ? (
              groups.map((group) => (
                <div className="shoe-combobox-group" key={group.brand} role="group" aria-label={group.brand}>
                  <h4>{group.brand}</h4>
                  {group.shoes.map((shoe) => (
                    <button
                      aria-selected={shoe.id === value}
                      className="shoe-combobox-option"
                      data-combobox-option="true"
                      key={shoe.id}
                      onClick={() => selectShoe(shoe.id)}
                      onMouseDown={(event) => event.preventDefault()}
                      role="option"
                      type="button"
                    >
                      <strong>{shoe.model}</strong>
                      <span>{shoe.brand}</span>
                    </button>
                  ))}
                </div>
              ))
            ) : (
              <p className="shoe-combobox-empty">ไม่พบรุ่นที่ตรงกับคำค้น</p>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}
