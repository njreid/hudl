// Package mockdata provides sample data that simulates database queries.
package mockdata

import "fmt"

// DashboardData contains all data for the dashboard view.
type DashboardData struct {
	RevenueFormatted string                   `json:"revenueFormatted"`
	ActiveUsers      int                      `json:"activeUsers"`
	SystemLoad       int                      `json:"systemLoad"`
	Transactions     []TransactionViewData    `json:"transactions"`
}

// TransactionViewData is a view-ready transaction with pre-formatted fields.
type TransactionViewData struct {
	ID              string `json:"id"`
	CustomerName    string `json:"customerName"`
	CustomerEmail   string `json:"customerEmail"`
	AmountFormatted string `json:"amountFormatted"`
	Status          string `json:"status"`
}

// GetDashboardData returns mock dashboard data.
func GetDashboardData() DashboardData {
	return DashboardData{
		RevenueFormatted: "$12,345.67",
		ActiveUsers:      2847,
		SystemLoad:       73,
		Transactions: []TransactionViewData{
			{
				ID:              "TXN-001",
				CustomerName:    "Alice Johnson",
				CustomerEmail:   "alice@example.com",
				AmountFormatted: "$150.00",
				Status:          "active",
			},
			{
				ID:              "TXN-002",
				CustomerName:    "Bob Smith",
				CustomerEmail:   "bob@example.com",
				AmountFormatted: "$75.00",
				Status:          "pending",
			},
			{
				ID:              "TXN-003",
				CustomerName:    "Carol White",
				CustomerEmail:   "carol@example.com",
				AmountFormatted: "$320.00",
				Status:          "active",
			},
			{
				ID:              "TXN-004",
				CustomerName:    "David Brown",
				CustomerEmail:   "david@example.com",
				AmountFormatted: "$50.00",
				Status:          "failed",
			},
			{
				ID:              "TXN-005",
				CustomerName:    "Eve Davis",
				CustomerEmail:   "eve@example.com",
				AmountFormatted: "$899.00",
				Status:          "active",
			},
		},
	}
}

// LayoutData contains data for the app layout.
type LayoutData struct {
	Title      string `json:"title"`
	UserName   string `json:"userName"`
	IsLoggedIn bool   `json:"isLoggedIn"`
	Content    string `json:"content"`
}

// GetLayoutData returns layout data for an authenticated user.
func GetLayoutData(title, content string, loggedIn bool) LayoutData {
	data := LayoutData{
		Title:      title,
		IsLoggedIn: loggedIn,
		Content:    content,
	}
	if loggedIn {
		data.UserName = "John"
	}
	return data
}

// FeatureViewData is a view-ready feature.
type FeatureViewData struct {
	Icon        string `json:"icon"`
	Title       string `json:"title"`
	Description string `json:"description"`
	LinkUrl     string `json:"linkUrl"`
}

// GetFeatures returns mock feature data for marketing pages.
func GetFeatures() []FeatureViewData {
	return []FeatureViewData{
		{
			Icon:        "🚀",
			Title:       "Lightning Fast",
			Description: "Our WASM-powered templates render in microseconds, delivering exceptional performance.",
			LinkUrl:     "/docs/performance",
		},
		{
			Icon:        "🛡️",
			Title:       "Type Safe",
			Description: "Catch errors at compile time with full Go type integration.",
			LinkUrl:     "/docs/type-safety",
		},
		{
			Icon:        "💻",
			Title:       "Developer Friendly",
			Description: "Intuitive KDL syntax with LSP support for autocompletion and diagnostics.",
			LinkUrl:     "",
		},
		{
			Icon:        "🔌",
			Title:       "Easy Integration",
			Description: "Drop-in replacement for existing template engines.",
			LinkUrl:     "/docs/getting-started",
		},
		{
			Icon:        "🔒",
			Title:       "Secure by Default",
			Description: "Automatic HTML escaping and sandboxed WASM execution.",
			LinkUrl:     "",
		},
		{
			Icon:        "🔄",
			Title:       "Hot Reload",
			Description: "See changes instantly during development.",
			LinkUrl:     "/docs/hot-reload",
		},
	}
}

// FormData contains registration form data.
type FormData struct {
	Username       string `json:"username"`
	Email          string `json:"email"`
	TermsAccepted  bool   `json:"termsAccepted"`
	CsrfToken      string `json:"csrfToken"`
	UsernameError  string `json:"usernameError"`
	EmailError     string `json:"emailError"`
	PasswordError  string `json:"passwordError"`
}

// GetEmptyForm returns an empty registration form.
func GetEmptyForm(csrfToken string) FormData {
	return FormData{
		CsrfToken: csrfToken,
	}
}

// GetFormWithErrors returns a form with validation errors.
func GetFormWithErrors(csrfToken string) FormData {
	return FormData{
		Username:       "john",
		Email:          "invalid-email",
		TermsAccepted:  false,
		CsrfToken:      csrfToken,
		UsernameError:  "Username must be at least 4 characters",
		EmailError:     "Please enter a valid email address",
		PasswordError:  "Passwords do not match",
	}
}

// FormatMoney formats cents as a dollar string.
func FormatMoney(cents int64) string {
	dollars := float64(cents) / 100
	return fmt.Sprintf("$%.2f", dollars)
}
