import { createFileRoute } from "@tanstack/react-router";

import { catalog } from "../data/catalog";
import { ShoeCatalogPage } from "../features/shoes/ShoeCatalogPage";

export const Route = createFileRoute("/shoes/")({
  component: ShoesIndexRoute,
});

function ShoesIndexRoute() {
  return <ShoeCatalogPage catalog={catalog} />;
}
