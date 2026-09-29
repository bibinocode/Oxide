import { createFileRoute } from "@tanstack/react-router";
import { NotionImport } from "../features/admin/notion/NotionImport";

export const Route = createFileRoute("/admin/notion")({ component: NotionImport });
