import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

// /dashboard is the app's sole entrypoint (ADR-0010) — "/" only exists to redirect there.
export const load: PageLoad = () => {
  throw redirect(307, "/dashboard");
};
