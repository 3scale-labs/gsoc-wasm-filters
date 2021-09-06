package main

import (
	"encoding/xml"
	"fmt"
	"io"
	"net/http"
	"os"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type SingletonFlushTestSuite struct {
	suite.Suite
	backend      Backend
	ServiceID    string
	ServiceToken string
	AppIDs       []string
	PlanID       string
	Metrics      []Metric
}

func (suite *SingletonFlushTestSuite) SetupSuite() {
	suite.ServiceID = "test-service-id"
	suite.AppIDs = []string{"test-app-id-1", "test-app-id-2"}
	suite.ServiceToken = "test-service-token"
	suite.PlanID = "test-plan-id"
	suite.Metrics = []Metric{
		{"hits", "1", []UsageLimit{
			{Day, 10000},
		}},
		{"rq", "2", []UsageLimit{
			{Month, 10000},
		}},
	}

}

// Generates envoy config from the template.
func configureSingletonFlush(flushMode string, deltaStore string, periodical string, serviceID string, serviceToken string) error {
	configData := []byte(fmt.Sprintf(`{ "SingletonFlushMode": "%s" , 
										"SingletonPeriodicFlush": "%s", 
										"SingletonCapacity": "%s",
										"ServiceID": "%s",
										"ServiceToken": "%s"
									   }`,
		flushMode, periodical, deltaStore, serviceID, serviceToken))
	return GenerateConfig("temp.yaml", configData)
}

func TestSingletonFlushSuite(t *testing.T) {
	fmt.Println("Running SingletonFlushSuite")
	suite.Run(t, new(SingletonFlushTestSuite))
}

// This helper method returns the usage for the hits metric which is used for the tests.
func getApisonatorUsage(serviceID string, serviceToken string, appID string) (UsageReport, error) {
	client := &http.Client{}
	req, reqErr := http.NewRequest("GET", "http://127.0.0.1:3000/transactions/authorize.xml", nil)
	if reqErr != nil {
		fmt.Printf("reqErr: %v\n", reqErr)
		return UsageReport{}, reqErr
	}
	q := req.URL.Query()
	q.Add("service_token", serviceToken)
	q.Add("service_id", serviceID)
	q.Add("app_id", appID)
	req.URL.RawQuery = q.Encode()
	res, resErr := client.Do(req)
	if resErr != nil {
		fmt.Printf("resErr: %v\n", resErr)
		return UsageReport{}, resErr
	}
	fmt.Printf("Auth response: %v\n", res)
	response := AuthResponse{}
	bodyBytes, _ := io.ReadAll(res.Body)
	xmlErr := xml.Unmarshal(bodyBytes, &response)
	if xmlErr != nil {
		fmt.Printf("XML Error: %v\n", xmlErr)
	}
	fmt.Printf("XML: %v\n", response)
	for _, usage := range response.Usages {
		if usage.Metric == "hits" {
			return usage, nil
		}
	}
	return UsageReport{}, fmt.Errorf("Usages for the metric not found")
}

// Gets triggered after each test and runs the post-conditions like deleting apisonator services
// and deleting the temporary config file.
func (suite *SingletonFlushTestSuite) AfterTest(suiteName, testName string) {
	downErr := StopProxy()
	require.Nilf(suite.T(), downErr, "Error stopping proxy: %v", downErr)
	fmt.Println("Cleaning 3scale backend state")
	flushErr := suite.backend.Flush()
	if flushErr != nil {
		fmt.Printf("Error flushing the backend state: %v", flushErr)
		suite.backend.states = suite.backend.states[:0]
	}
	deleteErr := os.Remove("./temp.yaml")
	require.Nilf(suite.T(), deleteErr, "Error deleting temporary envoy.yaml")
}

// This helper method initializes services, apps, metrics and usages in the apisonator for the tests.
func (suite *SingletonFlushTestSuite) initializeApisonatorState() {
	require.Eventually(suite.T(), func() bool {
		fmt.Printf("Creating service with service_id: %s, service_token:%s", suite.ServiceID, suite.ServiceToken)
		serviceErr := suite.backend.Push("service", []interface{}{suite.ServiceID, suite.ServiceToken})
		if serviceErr != nil {
			return false
		}
		return true
	}, 4*time.Second, 500*time.Millisecond, "Error creating the service")

	for _, app := range suite.AppIDs {
		require.Eventually(suite.T(), func() bool {
			err := suite.backend.Push("app", []interface{}{suite.ServiceID, app, suite.PlanID})
			if err != nil {
				return false
			}
			return true
		}, 4*time.Second, 500*time.Millisecond, "Error creating the app")
	}

	require.Eventually(suite.T(), func() bool {
		err := suite.backend.Push("metrics", []interface{}{suite.ServiceID, &suite.Metrics})
		if err != nil {
			return false
		}
		return true
	}, 4*time.Second, 500*time.Millisecond, "Error adding metrics")

	require.Eventually(suite.T(), func() bool {
		err := suite.backend.Push("usage_limits", []interface{}{suite.ServiceID, suite.PlanID, &suite.Metrics})
		if err != nil {
			return false
		}
		return true
	}, 4*time.Second, 500*time.Millisecond, "Error adding usage limits")

}

// ------------------- Test cases begin here ---------------------------------

// This scenario tests the ContainerFlush scenario by sending requests and
// flushing them based on the delta store container size. For this scenario, delta
// store config is configured as 100.
func (suite *SingletonFlushTestSuite) TestSingletonContainerFlush() {
	// Pre-conditions before test
	suite.ServiceID = uuid.NewString()
	suite.ServiceToken = uuid.NewString()
	// Configure the envoy template with ContainerLimit flush.
	configErr := configureSingletonFlush("ContainerLimit", "100", "60", suite.ServiceID, suite.ServiceToken)
	require.Nilf(suite.T(), configErr, "Error configuring envoy.yaml for container flush : %v", configErr)
	// Create service, apps, metrics and usage limits in apisonator
	suite.initializeApisonatorState()
	// Start the proxy.
	upErr := StartProxy("./", "./temp.yaml")
	require.Nilf(suite.T(), upErr, "Error starting proxy: %v", upErr)
	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")
	// Test scenario begins here
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	q := req.URL.Query()
	q.Add("app_id", "test-app-id-1")
	req.URL.RawQuery = q.Encode()
	for i := 0; i < 4; i++ {
		res, _ := client.Do(req)
		fmt.Printf("Response: %v\n", res)
	}
	time.Sleep(3 * time.Second)
	require.Eventually(suite.T(), func() bool {
		usage, usageErr := getApisonatorUsage(suite.ServiceID, suite.ServiceToken, "test-app-id-1")
		if usageErr != nil {
			fmt.Printf("Error fetching apisonator usage: %v", usageErr)
			return false
		}
		if usage.Current == int64(4) {
			return true
		}
		return false
	}, 5*time.Second, 1*time.Second, "Invalid number for usages for the metric hits in apisonator")
}

// This scenario tests the PeriodicalFlush scenario by sending requests,
// wait for some time and check the apisonator usage. For this test periodical
// time limit is configured as 10s.
func (suite *SingletonFlushTestSuite) TestSingletonPeriodicalFlush() {
	// Pre-conditions before test
	suite.ServiceID = uuid.NewString()
	suite.ServiceToken = uuid.NewString()

	// Configure the envoy template with PeriodicalFlush. Flush period set as 10 seconds to check the apisonator values.
	configErr := configureSingletonFlush("Periodical", "100", "10", suite.ServiceID, suite.ServiceToken)
	require.Nilf(suite.T(), configErr, "Error configuring envoy.yaml for periodical flush: %v", configErr)
	suite.initializeApisonatorState()
	// Create service, apps, metrics and usage limits in apisonator
	upErr := StartProxy("./", "./temp.yaml")
	require.Nilf(suite.T(), upErr, "Error starting proxy: %v", upErr)
	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	q := req.URL.Query()
	q.Add("app_id", "test-app-id-1")
	req.URL.RawQuery = q.Encode()
	for i := 0; i < 5; i++ {
		res, _ := client.Do(req)
		if i == 0 {
			time.Sleep(5 * time.Second)
		}
		fmt.Printf("Response: %v\n", res)
	}
	require.Eventually(suite.T(), func() bool {
		usage, usageErr := getApisonatorUsage(suite.ServiceID, suite.ServiceToken, "test-app-id-1")
		if usageErr != nil {
			fmt.Printf("Error fetching apisonator usage: %v", usageErr)
			return false
		}
		if usage.Current == int64(5) {
			return true
		}
		return false
	}, 15*time.Second, 1*time.Second, "Invalid number for usages for the metric hits in apisonator")
}
