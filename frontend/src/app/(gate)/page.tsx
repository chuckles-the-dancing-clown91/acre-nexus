import { redirect } from "next/navigation";

// The public website hasn't been rebuilt yet; until it is, the root sends
// people to the console (which bounces signed-out visitors to /login).
export default function Home() {
  redirect("/console");
}
