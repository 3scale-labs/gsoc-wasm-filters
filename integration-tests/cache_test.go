package main

import (
	"encoding/json"
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

type CacheTestSuite struct {
	suite.Suite
	backend Backend
	client  http.Client
}

func (suite *CacheTestSuite) TestServiceManagementTimeout() {
	configData := []byte(fmt.Sprintf(`{
		"ClusterSocketAddress": "middleware",
		"ClusterSocketPort": "3001",
		"ServiceID": "%s",
		"ServiceToken": "%s"
	}`, uuid.NewString(), uuid.NewString()))
	GenerateConfig("temp.yaml", configData)
	StartProxy(".", "temp.yaml")
	StartMiddleware()
	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")

	// Initializing backend state is not required since we are testing for timeout
	req, errReq := http.NewRequest("GET", "http://localhost:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{"does-not-matter"},
	}

	res, resErr := suite.client.Do(req)
	require.Nilf(suite.T(), resErr, "Error sending the HTTP request: %v", resErr)

	var logs []string
	unmarshalErr := json.Unmarshal([]byte(res.Header["Filter-Logs"][0]), &logs)
	require.Nilf(suite.T(), unmarshalErr, "Error while unmarshaling: %v", unmarshalErr)

	patterns := []string{".*cache miss.*", ".*dispatch successful.*", ".*received response.*", ".*timeout.*"}
	patternsMatched := SerialSearch(logs, patterns)
	assert.Equal(suite.T(), true, patternsMatched, "All patterns are not matched! Logs: %v", logs)

	deleteErr := os.Remove("./temp.yaml")
	require.Nilf(suite.T(), deleteErr, "Error deleting temporary config file")
	StopProxy()
	StopMiddleware()
}

func (suite *CacheTestSuite) TestCacheHit() {
	// Apisonator state
	serviceID := uuid.NewString()
	serviceToken := uuid.NewString()
	appID := "test-app-id"
	userKey := "test-user-key"
	planID := "test-plan-id"
	metrics := []Metric{{"hits", "1", []UsageLimit{}}}

	// Initializing apisonator state
	serviceErr := suite.backend.Push("service", []interface{}{serviceID, serviceToken})
	require.Nilf(suite.T(), serviceErr, "Error creating a service: %v", serviceErr)
	appErr := suite.backend.Push("app", []interface{}{serviceID, appID, planID})
	require.Nilf(suite.T(), appErr, "Error creating an app: %v", appErr)
	userKeyErr := suite.backend.Push("user_key", []interface{}{serviceID, appID, userKey})
	require.Nilf(suite.T(), userKeyErr, "Error creating an user key: %v", userKeyErr)
	metricsErr := suite.backend.Push("metrics", []interface{}{serviceID, &metrics})
	require.Nilf(suite.T(), metricsErr, "Error adding metrics: %v", metricsErr)

	// Generating custom config and starting proxy with it
	configData := []byte(fmt.Sprintf(`{
		"ServiceID": "%s",
		"ServiceToken": "%s" 
	}`, serviceID, serviceToken))
	GenerateConfig("temp.yaml", configData)
	StartProxy(".", "temp.yaml")
	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")

	// Sending requests and doing pattern match based on the response
	req, reqErr := http.NewRequest("GET", "http://localhost:9095", nil)
	require.Nilf(suite.T(), reqErr, "Error while creating request: %v", reqErr)
	q := req.URL.Query()
	q.Add("api_key", userKey)
	req.URL.RawQuery = q.Encode()
	patterns1 := []string{".*cache miss.*", ".*dispatch successful.*", ".*received response.*"}
	patterns2 := []string{".*cache hit.*", ".*allowed to pass.*"}
	for i := 1; i <= 2; i++ {
		res, resErr := suite.client.Do(req)
		require.Nilf(suite.T(), resErr, "Error while sending the request: %v", resErr)

		var logs []string
		unmarshalErr := json.Unmarshal([]byte(res.Header["Filter-Logs"][0]), &logs)
		require.Nilf(suite.T(), unmarshalErr, "Error while unmarshaling: %v", unmarshalErr)

		var patternsMatched bool
		if i == 1 {
			patternsMatched = SerialSearch(logs, patterns1)
		} else {
			patternsMatched = SerialSearch(logs, patterns2)
		}
		assert.Equal(suite.T(), true, patternsMatched, "All patterns are not matched for the request#%d ! Logs: %v", i, logs)
	}

	// Cleanup
	flushErr := suite.backend.Flush()
	require.Nilf(suite.T(), flushErr, "Error while flushing apisonator state: %v", flushErr)
	deleteErr := os.Remove("./temp.yaml")
	require.Nilf(suite.T(), deleteErr, "Error deleting temporary config file")
	StopProxy()
}

func (suite *CacheTestSuite) TestRateLimitFlow() {
	// Apisonator state
	serviceID := uuid.NewString()
	serviceToken := uuid.NewString()
	appID := "test-app-id"
	planID := "test-plan-id"
	metrics := []Metric{{"hits", "1", []UsageLimit{{Day, 4}}}}

	// Initializing apisonator state
	serviceErr := suite.backend.Push("service", []interface{}{serviceID, serviceToken})
	require.Nilf(suite.T(), serviceErr, "Error creating a service: %v", serviceErr)
	appErr := suite.backend.Push("app", []interface{}{serviceID, appID, planID})
	require.Nilf(suite.T(), appErr, "Error creating an app: %v", appErr)
	metricsErr := suite.backend.Push("metrics", []interface{}{serviceID, &metrics})
	require.Nilf(suite.T(), metricsErr, "Error adding metrics: %v", metricsErr)
	limitErr := suite.backend.Push("usage_limits", []interface{}{serviceID, planID, &metrics})
	require.Nilf(suite.T(), limitErr, "Error adding limits to metrics: %v", limitErr)

	// Generating custom config and starting proxy with it
	configData := []byte(fmt.Sprintf(`{
		"ServiceID": "%s",
		"ServiceToken": "%s" 
	}`, serviceID, serviceToken))
	GenerateConfig("temp.yaml", configData)
	StartProxy(".", "temp.yaml")
	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")

	req, errReq := http.NewRequest("GET", "http://localhost:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{appID},
	}

	notLimitedPattern := []string{".*request is allowed.*"}
	rateLimitedPattern := []string{".*request is rate-limited"}
	for i := 1; i <= 5; i++ {
		res, resErr := suite.client.Do(req)
		require.Nilf(suite.T(), resErr, "Error while sending the request: %v", resErr)
		fmt.Printf("Response #%d: %v\n", i, res)

		// Allow proxy to process the request and make the test more predictable
		time.Sleep(300 * time.Millisecond)

		var logs []string
		unmarshalErr := json.Unmarshal([]byte(res.Header["Filter-Logs"][0]), &logs)
		require.Nilf(suite.T(), unmarshalErr, "Error while unmarshaling: %v", unmarshalErr)

		var patternsMatched bool
		if i < 5 {
			patternsMatched = SerialSearch(logs, notLimitedPattern)
		} else {
			patternsMatched = SerialSearch(logs, rateLimitedPattern)
		}
		assert.Equal(suite.T(), true, patternsMatched, "All patterns are not matched for the request#%d ! Logs: %v", i, logs)
	}

	// Cleanup
	flushErr := suite.backend.Flush()
	require.Nilf(suite.T(), flushErr, "Error while flushing apisonator state: %v", flushErr)
	deleteErr := os.Remove("./temp.yaml")
	require.Nilf(suite.T(), deleteErr, "Error deleting temporary config file")
	StopProxy()
}

func TestCacheSuite(t *testing.T) {
	fmt.Println("Running ==TestCacheSuite==")
	suite.Run(t, new(CacheTestSuite))
}
