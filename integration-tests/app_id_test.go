package main

import (
	"fmt"
	"net/http"
	"testing"

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
	err := StartProxy("./", "./envoy.yaml")
	require.Nilf(suite.T(), err, "Error starting proxy: %v", err)
	// Initializing 3scale backend state
	suite.AppID = "test_app_id"
	suite.AppKey = "test_app_key"
	suite.UserKey = "test_user_key"
	suite.ServiceID = "test_service_id"
	suite.PlanID = "test_plan_id"
	suite.ServiceToken = "test_service_token"
	suite.metrics = []Metric{
		{"hits", "1", []UsageLimit{
			{Day, 10000},
		}},
		{"rq", "2", []UsageLimit{
			{Month, 10000},
		}},
	}
	serviceErr := suite.backend.Push("service", []interface{}{suite.ServiceID, suite.ServiceToken})
	require.Nilf(suite.T(), serviceErr, "Error: %v", serviceErr)

	appErr := suite.backend.Push("app", []interface{}{suite.ServiceID, suite.AppID, suite.PlanID})
	require.Nilf(suite.T(), appErr, "Error: %v", appErr)

	userErr := suite.backend.Push("user_key", []interface{}{suite.ServiceID, suite.AppID, suite.UserKey})
	require.Nilf(suite.T(), userErr, "Error: %v", userErr)

	appKeyErr := suite.backend.Push("app_key", []interface{}{suite.ServiceID, suite.AppID, suite.AppKey})
	require.Nilf(suite.T(), appKeyErr, "Error: %v", appKeyErr)

	metricsErr := suite.backend.Push("metrics", []interface{}{suite.ServiceID, &suite.metrics})
	require.Nilf(suite.T(), metricsErr, "Error: %v", metricsErr)

	usageErr := suite.backend.Push("usages", []interface{}{suite.ServiceID, suite.PlanID, &suite.metrics})
	require.Nilf(suite.T(), usageErr, "Error: %v", usageErr)

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
	fmt.Println("Cleaning 3scale backend state")
	flushErr := suite.backend.Flush()
	require.Nilf(suite.T(), flushErr, "Error: %v", flushErr)
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
		"x-app-id":  []string{"wrong_app_id"},
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
	q.Add("api_key", "wrong_user_key")
	req.URL.RawQuery = q.Encode()
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 403, res.StatusCode, "Invalid http response code for user_key forbidden test: %v", res.StatusCode)
}

func (suite *AppCredentialTestSuite) TestUnlimitedUserKey() {
	// Add a new unlimited app
	if err := suite.backend.Push("app", []interface{}{suite.ServiceID, "unlimited_app_id", suite.PlanID}); err != nil {
		require.Nilf(suite.T(), err, "Error adding an application: %v", err)
	}
	if err := suite.backend.Push("user_key", []interface{}{suite.ServiceID, "unlimited_app_id", suite.UserKey}); err != nil {
		require.Nilf(suite.T(), err, "Error adding a user key: %v", err)
	}
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

	if err := suite.backend.Pop(); err != nil {
		require.Nilf(suite.T(), err, "Failed to delete Application's user key: %v", err)
	}
	if err := suite.backend.Pop(); err != nil {
		require.Nilf(suite.T(), err, "Failed to delete applications: %v", err)
	}
}

func (suite *AppCredentialTestSuite) TestUnlimitedAppId() {
	// Add a new unlimited app
	if err := suite.backend.Push("app", []interface{}{suite.ServiceID, "unlimited_app_id", suite.PlanID}); err != nil {
		require.Nilf(suite.T(), err, "Error adding an application: %v", err)
	}

	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{"unlimited_app_id"},
	}
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for appId unlimited test: %d", res.StatusCode)

	if err := suite.backend.Pop(); err != nil {
		require.Nilf(suite.T(), err, "Failed to delete applications: %v", err)
	}
}
