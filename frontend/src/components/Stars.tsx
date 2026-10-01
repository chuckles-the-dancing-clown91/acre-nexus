// A five-star rating, filled to the nearest half.

export function Stars({
  rating,
  size = 14,
}: {
  rating: number;
  size?: number;
}) {
  const full = Math.round(rating * 2) / 2;
  return (
    <span
      role="img"
      aria-label={`${rating.toFixed(1)} out of 5 stars`}
      className="inline-flex gap-0.5"
    >
      {[1, 2, 3, 4, 5].map((i) => (
        <svg
          key={i}
          width={size}
          height={size}
          viewBox="0 0 20 20"
          aria-hidden="true"
        >
          <defs>
            <linearGradient id={`half-${i}-${size}`}>
              <stop offset="50%" stopColor="#f5b301" />
              <stop offset="50%" stopColor="#d6d3cb" />
            </linearGradient>
          </defs>
          <path
            d="M10 1.5l2.6 5.5 6 .8-4.4 4.2 1.1 6-5.3-2.9-5.3 2.9 1.1-6L1.4 7.8l6-.8z"
            fill={
              full >= i
                ? "#f5b301"
                : full >= i - 0.5
                  ? `url(#half-${i}-${size})`
                  : "#d6d3cb"
            }
          />
        </svg>
      ))}
    </span>
  );
}
