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
	service_id string
	service_token string
	app_id string
	app_key string
	user_key string
	plan_id string
	metrics []Metric
}

func (suite *AppCredentialTestSuite) SetupSuite() {
	if err := StartContainers("./configs/app-id/docker-compose.yaml"); err != nil {
		fmt.Printf("Error: %v", err)
		suite.Errorf(err, "Error starting docker-compose: %v")
	}
	// Initializing 3scale backend state
	suite.app_id = "test_app_id"
	suite.app_key = "test_app_key"
	suite.user_key = "test_user_key"
	suite.service_id = "test_service_id"
	suite.plan_id = "test_plan_id"
	suite.service_token = "test_service_token"
	suite.metrics = []Metric {
		Metric {"hits", "1", []UsageLimit {
			{ Day, 10000,},
		}},
		Metric {"rq", "2", []UsageLimit {
			{ Month, 10000, },
		}},
	}
	if err := CreateService(suite.service_id, suite.service_token); err != nil {
		suite.Errorf(err, "Error creating a service: %v")
		return
	}
	if err := AddApplication(suite.service_id, suite.app_id, suite.plan_id); err != nil {
		suite.Errorf(err, "Error adding an application: %v")
		return
	}
	if err := AddUserKey(suite.service_id, suite.app_id, suite.user_key); err != nil {
		suite.Errorf(err, "Error adding a user key: %v")
	}
	if err := AddApplicationKey(suite.service_id, suite.app_id, suite.app_key); err != nil {
		suite.Errorf(err, "Error adding application key: %v")
		return
	}
	if err := AddMetrics(suite.service_id, &suite.metrics); err != nil {
		suite.Errorf(err, "Error adding metrics: %v")
		return
	}
	if err := UpdateUsageLimits(suite.service_id, suite.plan_id, &suite.metrics); err != nil {
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
	if err := DeleteService(suite.service_id, suite.service_token); err != nil {
		suite.Errorf(err, "Failed to delete service: %v")
	}
	if err := DeleteApplication(suite.service_id, suite.app_id); err != nil {
		suite.Errorf(err, "Failed to delete applications: %v")
	}
	if err := DeleteApplicationKey(suite.service_id, suite.app_id, suite.app_key); err != nil {
		suite.Errorf(err, "Failed to delete Application key: %v")
	}
	if err := DeleteUserKey(suite.service_id, suite.app_id, suite.user_key); err != nil {
		suite.Errorf(err, "Failed to delete Application's user key: %v")
	}
	if err := DeleteMetrics(suite.service_id, &suite.metrics); err != nil {
		suite.Errorf(err, "Failed to delete metrics: %v")
	}
	if err := DeleteUsageLimits(suite.service_id, suite.plan_id, &suite.metrics); err != nil {
		suite.Errorf(err, "Failed to delete usage limits")
	}
	fmt.Println("Stopping AppCredentialTestSuite")
	errStop := StopContainers("./configs/app-id/docker-compose.yaml")
	if errStop != nil {
		fmt.Printf("Error stopping : %v", errStop)
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
		"x-app-key": []string{suite.app_key},
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

func (suite *AppCredentialTestSuite) TestUnlimitedUserKey() {
 	client := &http.Client{}
 	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
 	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
 	q := req.URL.Query()
 	q.Add("api_key", "10e81d5c065a537b05ab7d78a7156fc5")
 	req.URL.RawQuery = q.Encode()
 	res, errHTTP := client.Do(req)
 	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
 	fmt.Printf("Response: %v", res)
 	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for user_key unlimited test: %v", res.StatusCode)
 }

func (suite *AppCredentialTestSuite) TestUnlimitedAppId() {
 	client := &http.Client{}
 	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
 	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
 	req.Header = http.Header{
 		"Host":      []string{"localhost"},
		"x-app-id":  []string{"fbcdf529"},
 		"x-app-key": []string{"616b9f05e588cd32f9c0db17dbe23781"},
 	}
 	res, errHTTP := client.Do(req)
 	require.Nilf(suite.T(), errHTTP, "Error sending the HTTP request: %v", errHTTP)
 	fmt.Printf("Response: %v", res)
 	assert.Equal(suite.T(), 200, res.StatusCode, "Invalid http response code for appId unlimited test: %d", res.StatusCode)
}
