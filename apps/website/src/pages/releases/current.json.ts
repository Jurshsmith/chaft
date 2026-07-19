import { currentRelease } from "../../data/releases";

export const prerender = true;

export function GET() {
  return new Response(`${JSON.stringify(currentRelease, null, 2)}\n`, {
    headers: {
      "Content-Type": "application/json; charset=utf-8",
    },
  });
}
