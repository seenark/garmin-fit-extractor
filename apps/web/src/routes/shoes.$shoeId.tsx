import { createFileRoute } from "@tanstack/react-router";

import { catalog } from "../data/catalog";
import { ShoeDetailPage } from "../features/shoes/ShoeDetailPage";

export const Route = createFileRoute("/shoes/$shoeId")({
  component: ShoeDetailRoute,
});

function ShoeDetailRoute() {
  const { shoeId } = Route.useParams();
  const shoe = catalog.shoes.find((candidate) => candidate.id === shoeId);
  return <ShoeDetailPage catalog={catalog} shoe={shoe} />;
}
