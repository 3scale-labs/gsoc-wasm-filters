package main

import (
	"fmt"
	"net/http"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type AppCredentialTestSuite struct {
	suite.Suite
}

func (suite *AppCredentialTestSuite) SetupSuite() {
	err := StartContainers("./configs/app-id/docker-compose.yaml")
	time.Sleep(10 * time.Second)
	if err != nil {
		fmt.Printf("Error: %v", err)
		suite.Errorf(err, "Error starting docker-compose: %v")
	}
}

func TestAppCredentialSuite(t *testing.T) {
	fmt.Println("Running AppCredentialTestSuite")
	suite.Run(t, new(AppCredentialTestSuite))
}

func (suite *AppCredentialTestSuite) TearDownSuite() {
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
		"x-app-id":  []string{"23f118bf"},
		"x-app-key": []string{"44d128988763aee1b0ff0691f9686f7e"},
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
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
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
