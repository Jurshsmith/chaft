import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";

import { docIdFromEntry } from "./lib/docs";

const docs = defineCollection({
  loader: glob({
    pattern: "**/*.md",
    base: "../../guides/public",
    generateId: ({ entry }) => docIdFromEntry(entry),
  }),
  schema: z.object({
    title: z.string().trim().min(1),
    description: z.string().trim().min(1),
    section: z.enum(["getting-started", "concepts", "development", "reference"]),
    order: z.number().int().nonnegative(),
    audience: z.enum(["users", "contributors", "operators"]),
    status: z.enum(["preview", "stable", "deprecated"]),
    draft: z.boolean().default(false),
    navTitle: z.string().trim().min(1).optional(),
  }),
});

export const collections = { docs };
