import { integer, sqliteTable, text } from "drizzle-orm/sqlite-core";

export const mediaItems = sqliteTable("media_items", {
  id: text("id").primaryKey(),
  title: text("title").notNull(),
  mediaType: text("media_type").notNull(),
  year: integer("year"),
  posterPath: text("poster_path"),
  createdAt: integer("created_at", { mode: "timestamp_ms" }).notNull(),
  updatedAt: integer("updated_at", { mode: "timestamp_ms" }).notNull()
});
