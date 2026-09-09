import { Outlet, createFileRoute, useLocation } from "@tanstack/react-router";

import { Route as RootRoute, SignInScreen } from "./__root";

export const Route = createFileRoute("/_authenticated")({
  component: AuthenticatedLayout,
});

function AuthenticatedLayout() {
  const { user } = RootRoute.useLoaderData();
  const { authError } = RootRoute.useSearch();
  const { pathname } = useLocation();
  const isPublicRoute = pathname === "/" || pathname === "/upload";

  if (!user && !isPublicRoute) {
    const isHistoryRoute = pathname === "/history";
    return (
      <SignInScreen
        authError={authError === "AUTH_FAILED"}
        description={
          isHistoryRoute
            ? "เข้าสู่ระบบเพื่อเปิด ดูรายละเอียด และจัดการข้อมูลการวิ่งที่บันทึกไว้"
            : undefined
        }
        eyebrow={isHistoryRoute ? "RUNS / ประวัติการวิ่ง" : undefined}
        title={isHistoryRoute ? "ดูประวัติการวิ่งของคุณ" : undefined}
      />
    );
  }

  return <Outlet />;
}
