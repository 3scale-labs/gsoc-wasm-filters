package main

import (
	"fmt"
	"net/http"
	"os"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type AppCredentialTestSuite struct {
	suite.Suite
	backend      Backend
	ServiceID    string
	ServiceToken string
	AppID        string
	AppKey       string
	UserKey      string
	PlanID       string
	metrics      []Metric
}

func (suite *AppCredentialTestSuite) SetupSuite() {
	// Initializing 3scale backend state
	suite.AppID = "test-app-id"
	suite.AppKey = "test-app-key"
	suite.UserKey = "test-user-key"
	suite.ServiceID = uuid.NewString()
	suite.PlanID = "test-plan-id"
	suite.ServiceToken = uuid.NewString()
	suite.metrics = []Metric{
		{"hits", "1", []UsageLimit{
			{Day, 10000},
		}},
		{"rq", "2", []UsageLimit{
			{Month, 10000},
		}},
	}

	require.Eventually(suite.T(), func() bool {
		serviceErr := suite.backend.Push("service", []interface{}{suite.ServiceID, suite.ServiceToken})
		if serviceErr != nil {
			fmt.Printf("Error creating the service: %v", serviceErr)
			return false
		}
		return true
	}, 5*time.Second, 1*time.Second)

	require.Eventually(suite.T(), func() bool {
		appErr := suite.backend.Push("app", []interface{}{suite.ServiceID, suite.AppID, suite.PlanID})
		if appErr != nil {
			fmt.Printf("Error creating the service: %v", appErr)
			return false
		}
		return true
	}, 5*time.Second, 1*time.Second)

	require.Eventually(suite.T(), func() bool {
		userErr := suite.backend.Push("user_key", []interface{}{suite.ServiceID, suite.AppID, suite.UserKey})
		if userErr != nil {
			fmt.Printf("Error creating the service: %v", userErr)
			return false
		}
		return true
	}, 5*time.Second, 1*time.Second)

	require.Eventually(suite.T(), func() bool {
		appKeyErr := suite.backend.Push("app_key", []interface{}{suite.ServiceID, suite.AppID, suite.AppKey})
		if appKeyErr != nil {
			fmt.Printf("Error creating the service: %v", appKeyErr)
			return false
		}
		return true
	}, 5*time.Second, 1*time.Second)

	require.Eventually(suite.T(), func() bool {
		metricsErr := suite.backend.Push("metrics", []interface{}{suite.ServiceID, &suite.metrics})
		if metricsErr != nil {
			fmt.Printf("Error creating the service: %v", metricsErr)
			return false
		}
		return true
	}, 5*time.Second, 1*time.Second)

	require.Eventually(suite.T(), func() bool {
		usageErr := suite.backend.Push("usage_limits", []interface{}{suite.ServiceID, suite.PlanID, &suite.metrics})
		if usageErr != nil {
			fmt.Printf("Error creating the service: %v", usageErr)
			return false
		}
		return true
	}, 5*time.Second, 1*time.Second)

	configErr := configureAppCredentialConfig(suite.ServiceID, suite.ServiceToken)
	require.Nilf(suite.T(), configErr, "Error configuring the envoy.yaml for AppCredentialTest")

	err := StartProxy("./", "./temp.yaml")
	require.Nilf(suite.T(), err, "Error starting proxy: %v", err)
	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")

}

func TestAppCredentialSuite(t *testing.T) {
	fmt.Println("Running AppCredentialTestSuite")
	suite.Run(t, new(AppCredentialTestSuite))
}

func (suite *AppCredentialTestSuite) TearDownSuite() {
	fmt.Println("Stopping AppCredentialTestSuite")
	if err := StopProxy(); err != nil {
		fmt.Printf("Error stoping docker: %v", err)
	}
	// Delete temporary config file
	deleteErr := os.Remove("./temp.yaml")
	require.Nilf(suite.T(), deleteErr, "Error deleting temporary envoy.yaml")

	fmt.Println("Cleaning 3scale backend state")
	flushErr := suite.backend.Flush()
	require.Nilf(suite.T(), flushErr, "Error: %v", flushErr)
}

// Generates envoy config from the template.
func configureAppCredentialConfig(serviceID string, serviceToken string) error {
	configData := []byte(fmt.Sprintf(`{ 
										"ServiceID": "%s",
										"ServiceToken": "%s"
									  }`,
		serviceID, serviceToken))
	return GenerateConfig("temp.yaml", configData)
}

func (suite *AppCredentialTestSuite) TestAppIdSuccess() {
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":      []string{"localhost"},
		"x-app-id":  []string{suite.AppID},
		"x-app-key": []string{suite.AppKey},
	}
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for appId success test: %v", res.StatusCode)
}

func (suite *AppCredentialTestSuite) TestAppIdForbidden() {
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":      []string{"localhost"},
		"x-app-id":  []string{"wrong-app-id"},
		"x-app-key": []string{suite.AppKey},
	}
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 403, res.StatusCode, "Invalid http response code for appId forbidden test: %d", res.StatusCode)
}

func (suite *AppCredentialTestSuite) TestUserKeySuccess() {
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	q := req.URL.Query()
	q.Add("api_key", suite.UserKey)
	req.URL.RawQuery = q.Encode()
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for user_key success test: %v", res.StatusCode)
}

func (suite *AppCredentialTestSuite) TestUserKeyForbidden() {
	client := &http.Client{}
	req, errReq := http.NewRequest(http.MethodGet, "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	q := req.URL.Query()
	q.Add("api_key", "wrong-user-key")
	req.URL.RawQuery = q.Encode()
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 403, res.StatusCode, "Invalid http response code for user_key forbidden test: %v", res.StatusCode)
}

func (suite *AppCredentialTestSuite) TestUnlimitedUserKey() {
	// Add a new unlimited app
	appErr := suite.backend.Push("app", []interface{}{suite.ServiceID, "unlimited-app-id", suite.PlanID})
	require.Nilf(suite.T(), appErr, "Error adding an application: %v", appErr)

	userKeyErr := suite.backend.Push("user_key", []interface{}{suite.ServiceID, "unlimited-app-id", suite.UserKey})
	require.Nilf(suite.T(), userKeyErr, "Error adding a user key: %v", userKeyErr)

	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	q := req.URL.Query()
	q.Add("api_key", suite.UserKey)
	req.URL.RawQuery = q.Encode()
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for user_key unlimited test: %v", res.StatusCode)

	deleteUserKeyErr := suite.backend.Pop()
	require.Nilf(suite.T(), deleteUserKeyErr, "Failed to delete Application's user key: %v", deleteUserKeyErr)

	deleteAppErr := suite.backend.Pop()
	require.Nilf(suite.T(), deleteAppErr, "Failed to delete applications: %v", deleteAppErr)
}

func (suite *AppCredentialTestSuite) TestUnlimitedAppId() {
	// Add a new unlimited app
	appErr := suite.backend.Push("app", []interface{}{suite.ServiceID, "unlimited-app-id", suite.PlanID})
	require.Nilf(suite.T(), appErr, "Error adding an application: %v", appErr)

	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{"unlimited-app-id"},
	}
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for appId unlimited test: %d", res.StatusCode)

	deleteAppErr := suite.backend.Pop()
	require.Nilf(suite.T(), deleteAppErr, "Failed to delete applications: %v", deleteAppErr)

}
