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
	ServiceID    string
	ServiceToken string
	AppID        string
	AppKey       string
	UserKey      string
	PlanID       string
	metrics      []Metric
}

func (suite *AppCredentialTestSuite) SetupSuite() {
	err := StartProxy("./")
	require.Nilf(suite.T(), err, "Error starting docker-compose: %v", err)
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
	if err := CreateService(suite.ServiceID, suite.ServiceToken); err != nil {
		suite.Errorf(err, "Error creating a service: %v")
		return
	}
	if err := AddApplication(suite.ServiceID, suite.AppID, suite.PlanID); err != nil {
		suite.Errorf(err, "Error adding an application: %v")
		return
	}
	if err := AddUserKey(suite.ServiceID, suite.AppID, suite.UserKey); err != nil {
		suite.Errorf(err, "Error adding a user key: %v")
	}
	if err := AddApplicationKey(suite.ServiceID, suite.AppID, suite.AppKey); err != nil {
		suite.Errorf(err, "Error adding application key: %v")
		return
	}
	if err := AddMetrics(suite.ServiceID, &suite.metrics); err != nil {
		suite.Errorf(err, "Error adding metrics: %v")
		return
	}
	if err := UpdateUsageLimits(suite.ServiceID, suite.PlanID, &suite.metrics); err != nil {
		suite.Errorf(err, "Error updating usage limits: %v")
		return
	}
}

func TestAppCredentialSuite(t *testing.T) {
	fmt.Println("Running AppCredentialTestSuite")
	suite.Run(t, new(AppCredentialTestSuite))
}

func (suite *AppCredentialTestSuite) TearDownSuite() {
	fmt.Println("Cleaning 3scale backend state")
	if err := DeleteService(suite.ServiceID, suite.ServiceToken); err != nil {
		suite.Errorf(err, "Failed to delete service: %v")
	}
	if err := DeleteApplication(suite.ServiceID, suite.AppID); err != nil {
		suite.Errorf(err, "Failed to delete applications: %v")
	}
	if err := DeleteApplicationKey(suite.ServiceID, suite.AppID, suite.AppKey); err != nil {
		suite.Errorf(err, "Failed to delete Application key: %v")
	}
	if err := DeleteUserKey(suite.ServiceID, suite.AppID, suite.UserKey); err != nil {
		suite.Errorf(err, "Failed to delete Application's user key: %v")
	}
	if err := DeleteMetrics(suite.ServiceID, &suite.metrics); err != nil {
		suite.Errorf(err, "Failed to delete metrics: %v")
	}
	if err := DeleteUsageLimits(suite.ServiceID, suite.PlanID, &suite.metrics); err != nil {
		suite.Errorf(err, "Failed to delete usage limits")
	}
	fmt.Println("Stopping AppCredentialTestSuite")

	if err := StopProxy(); err != nil {
		fmt.Printf("Error stoping docker: %v", err)
	}
}

func (suite *AppCredentialTestSuite) TestAppIdSuccess() {
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":      []string{"localhost"},
		"x-app-id":  []string{"23f118be"},
		"x-app-key": []string{"44d128988763aee1b0ff0691f9686f7e"},
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
	q.Add("api_key", "04fa57ba1e465fb53fddc21ead8a7fc0")
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
	q.Add("api_key", "04fa57ba1e465fb53fddc21ead8a7fc1")
	req.URL.RawQuery = q.Encode()
	res, errHTTP := client.Do(req)
	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
	fmt.Printf("Response: %v", res)
	assert.Equal(suite.T(), 403, res.StatusCode, "Invalid http response code for user_key forbidden test: %v", res.StatusCode)
}

// func (suite *AppCredentialTestSuite) TestUnlimitedUserKey() {
// 	client := &http.Client{}
// 	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
// 	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
// 	q := req.URL.Query()
// 	q.Add("api_key", "10e81d5c065a537b05ab7d78a7156fc5")
// 	req.URL.RawQuery = q.Encode()
// 	res, errHTTP := client.Do(req)
// 	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
// 	fmt.Printf("Response: %v", res)
// 	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for user_key unlimited test: %v", res.StatusCode)
// }

// func (suite *AppCredentialTestSuite) TestUnlimitedAppId() {
// 	client := &http.Client{}
// 	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
// 	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
// 	req.Header = http.Header{
// 		"Host":      []string{"localhost"},
// 		"x-app-id":  []string{"fbcdf529"},
// 		"x-app-key": []string{"616b9f05e588cd32f9c0db17dbe23781"},
// 	}
// 	res, errHTTP := client.Do(req)
// 	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
// 	fmt.Printf("Response: %v", res)
// 	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for appId unlimited test: %d", res.StatusCode)
// }
