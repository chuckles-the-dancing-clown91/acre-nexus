// The business profile and its Google reviews (roadmap area 5).

import { DEFAULT_TENANT, request } from "@/lib/api";

export interface Business {
  business_name: string | null;
  phone: string | null;
  email: string | null;
  website: string | null;
  address: string | null;
  hours: string | null;
  description: string | null;
  facebook_url: string | null;
  instagram_url: string | null;
  yelp_url: string | null;
  nextdoor_url: string | null;
  google_place_id: string | null;
  google_place_name: string | null;
  google_review_url: string | null;
  review_link: string | null;
  show_reviews: boolean;
  min_rating: number;
  max_reviews: number;
  refresh_minutes: number;
  embed_enabled: boolean;
  embed_origins: string | null;
  google_key_set: boolean;
  google_live: boolean;
  updated_at: string | null;
}

export type BusinessInput = Partial<
  Omit<
    Business,
    "review_link" | "google_key_set" | "google_live" | "updated_at"
  >
>;

export interface PlaceCandidate {
  place_id: string;
  name: string;
  address: string;
  rating: number | null;
  count: number;
  maps_url: string;
}

export interface Review {
  author: string;
  author_url: string;
  photo_url: string;
  rating: number;
  text: string;
  when: string;
  review_url: string;
}

export interface PlaceInfo {
  place_id: string;
  name: string;
  address: string;
  rating: number | null;
  count: number;
  maps_url: string;
  reviews: Review[];
  simulated: boolean;
}

export interface PublicReviews {
  business: string;
  rating: number | null;
  count: number;
  maps_url: string;
  write_url: string;
  reviews: Review[];
}

export const MAPS_KEY = "google.maps_api_key";

export const business = {
  get: () => request<Business>("/business-profile", { auth: true }),
  save: (body: BusinessInput) =>
    request<Business>("/business-profile", {
      method: "PUT",
      auth: true,
      body,
    }),
  search: (q: string) =>
    request<PlaceCandidate[]>(
      `/business-profile/google/search?q=${encodeURIComponent(q)}`,
      { auth: true }
    ),
  place: (refresh = false) =>
    request<PlaceInfo>(
      `/business-profile/google/place${refresh ? "?refresh=true" : ""}`,
      { auth: true }
    ),
  publicReviews: (tenant: string = DEFAULT_TENANT) =>
    request<PublicReviews>("/public/reviews", { tenant }),
  embedConfig: (tenant: string = DEFAULT_TENANT) =>
    request<{
      enabled: boolean;
      allowed_origins: string[];
      business: string;
    }>("/public/embed-config", { tenant }),
};
