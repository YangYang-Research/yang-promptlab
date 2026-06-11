type SearchInputProps = {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
};

export function SearchInput({
  value,
  onChange,
  placeholder = "Search…",
}: SearchInputProps) {
  return (
    <div className="search-input">
      <svg className="search-input__icon" viewBox="0 0 20 20" aria-hidden="true">
        <path
          d="M8.5 3a5.5 5.5 0 014.383 8.823l3.896 3.897a.75.75 0 11-1.06 1.06l-3.897-3.896A5.5 5.5 0 118.5 3zm0 1.5a4 4 0 100 8 4 4 0 000-8z"
          fill="currentColor"
        />
      </svg>
      <input
        type="search"
        className="search-input__field"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={placeholder}
      />
    </div>
  );
}
