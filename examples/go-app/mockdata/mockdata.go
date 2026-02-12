// Package mockdata provides sample data that simulates database queries,
// returning generated proto message types from pkg/hudl/pb.
package mockdata

import (
	"fmt"

	pb "github.com/njreid/hudl/pkg/hudl/pb"
)

// GetDashboardData returns mock dashboard data.
func GetDashboardData() *pb.DashboardData {
	return &pb.DashboardData{
		RevenueFormatted: "$12,345.67",
		ActiveUsers:      2847,
		SystemLoad:       73,
		Transactions: []*pb.Transaction{
			{
				Id:              "TXN-001",
				CustomerName:    "Alice Johnson",
				CustomerEmail:   "alice@example.com",
				AmountFormatted: "$150.00",
				Status:          pb.TransactionStatus_STATUS_ACTIVE,
			},
			{
				Id:              "TXN-002",
				CustomerName:    "Bob Smith",
				CustomerEmail:   "bob@example.com",
				AmountFormatted: "$75.00",
				Status:          pb.TransactionStatus_STATUS_PENDING,
			},
			{
				Id:              "TXN-003",
				CustomerName:    "Carol White",
				CustomerEmail:   "carol@example.com",
				AmountFormatted: "$320.00",
				Status:          pb.TransactionStatus_STATUS_ACTIVE,
			},
			{
				Id:              "TXN-004",
				CustomerName:    "David Brown",
				CustomerEmail:   "david@example.com",
				AmountFormatted: "$50.00",
				Status:          pb.TransactionStatus_STATUS_FAILED,
			},
			{
				Id:              "TXN-005",
				CustomerName:    "Eve Davis",
				CustomerEmail:   "eve@example.com",
				AmountFormatted: "$899.00",
				Status:          pb.TransactionStatus_STATUS_ACTIVE,
			},
		},
	}
}

// GetLayoutData returns layout data for wrapping content.
func GetLayoutData(title, content string, loggedIn bool) *pb.LayoutData {
	data := &pb.LayoutData{
		Title:      title,
		IsLoggedIn: loggedIn,
		Content:    content,
	}
	if loggedIn {
		data.UserName = "John"
	}
	return data
}

// GetFeatures returns mock feature data for marketing pages.
func GetFeatures() *pb.FeatureListData {
	return &pb.FeatureListData{
		Features: []*pb.Feature{
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
		},
	}
}

// GetEmptyForm returns an empty registration form.
func GetEmptyForm(csrfToken string) *pb.RegistrationFormData {
	return &pb.RegistrationFormData{
		CsrfToken: csrfToken,
	}
}

// GetFormWithErrors returns a form with validation errors.
func GetFormWithErrors(csrfToken string) *pb.RegistrationFormData {
	return &pb.RegistrationFormData{
		Username:      "john",
		Email:         "invalid-email",
		TermsAccepted: false,
		CsrfToken:     csrfToken,
		UsernameError: "Username must be at least 4 characters",
		EmailError:    "Please enter a valid email address",
		PasswordError: "Passwords do not match",
	}
}

// FormatMoney formats cents as a dollar string.
func FormatMoney(cents int64) string {
	dollars := float64(cents) / 100
	return fmt.Sprintf("$%.2f", dollars)
}
