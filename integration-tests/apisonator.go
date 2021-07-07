package main

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"net/http"
)

// These credentials should match those mentioned in the ci.yaml.
const (
	INTERNAL_USER   = "root"
	INTERNAL_PASS   = "root"
	PORT            = "3000"
	IP_ADDRESS      = "0.0.0.0"
	INTERNAL_PREFIX = "/internal"
	INTERNAL_URL    = "http://" + INTERNAL_USER + ":" + INTERNAL_PASS + "@" + IP_ADDRESS + ":" + PORT + INTERNAL_PREFIX
)

type Period string

const (
	Minute   Period = "minute"
	Hour     Period = "hour"
	Day      Period = "day"
	Week     Period = "week"
	Month    Period = "month"
	Year     Period = "year"
	Eternity Period = "eternity"
)

func (p Period) String() string {
	return string(p)
}

type UsageLimit struct {
	period Period
	value  int
}

type Metric struct {
	name   string
	id     string
	limits []UsageLimit
}

func CreateService(service_id string, service_token string) error {
	client := &http.Client{}
	// creating service with specified service_id
	header_data := []byte(fmt.Sprintf(`
		{ 
			"service": {
				"id": "%s",
				"state": "active"
			}
		}`, service_id))
	req, err := http.NewRequest("POST", INTERNAL_URL+"/services/", bytes.NewBuffer(header_data))
	if err != nil {
		fmt.Printf("Error while creating HTTP request: %v", err)
		return err
	}
	res, err := client.Do(req)
	if err != nil {
		fmt.Printf("Error sending the HTTP request: %v", err)
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to create a new service(id: %s)", service_id)
	}

	// adding a service_token to previously created service
	header_data = []byte(fmt.Sprintf(`
		{ 
			"service_tokens": {
				"%s": {
					"service_id": "%s"
				}
			}
		}`, service_token, service_id))
	url := INTERNAL_URL + "/service_tokens/"
	res, err = executeHttpRequest(http.MethodPost, url, &header_data)
	if err != nil {
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to create a service id (%s) and token pair (%s)", service_id, service_token)
	}
	return nil
}

func DeleteService(service_id string, service_token string) error {
	url := INTERNAL_URL + "/services/" + service_id
	res, err := executeHttpRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the service(id: %s)", service_id)
	}
	header_data := []byte(fmt.Sprintf(`
		{ 
			"service_tokens": [{
				"service_token": %s,
				"service_id": %s
			}]
		}`, service_token, service_id))
	url = INTERNAL_URL + "/service_tokens/"
	res, err = executeHttpRequest(http.MethodDelete, url, &header_data)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the service id (%s) and token pair (%s)", service_id, service_token)
	}
	return nil
}

// Creating a new application associated with 'service_id', 'app_id' and 'plan_id'
func AddApplication(service_id string, app_id string, plan_id string) error {
	header_data := []byte(fmt.Sprintf(`
		{ 
			"application": {
				"service_id": "%s",
				"id": "%s",
				"plan_id": "%s",
				"plan_name": "basic",
				"state": "active"
			}
		}`, service_id, app_id, plan_id))
	url := INTERNAL_URL + "/services/" + service_id + "/applications/" + app_id
	res, err := executeHttpRequest(http.MethodPost, url, &header_data)
	if err != nil {
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to create an application(id: %s)", app_id)
	}
	return nil
}

func DeleteApplication(service_id string, app_id string) error {
	url := INTERNAL_URL + "/services/" + service_id + "/applications" + app_id
	res, err := executeHttpRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the application(service_id: %s, app_id: %s)", service_id, app_id)
	}
	return nil
}

// Add key to the application identified by 'service_id' and 'app_id'
func AddApplicationKey(service_id string, app_id string, key string) error {
	header_data := []byte(fmt.Sprintf(`
		{ 
			"application_key": {
				"value": "%s"
			}
		}`, key))
	url := INTERNAL_URL + "/services/" + service_id + "/applications/" + app_id + "/keys"
	res, err := executeHttpRequest(http.MethodPost, url, &header_data)
	if err != nil {
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to add an application key(app_id: %s; key: %s)", app_id, key)
	}
	return nil
}

func DeleteApplicationKey(service_id string, app_id string, key string) error {
	url := INTERNAL_URL + "/services/" + service_id + "/applications" + app_id + "/keys/" + key
	res, err := executeHttpRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the application key(app_id: %s, key: %s)", app_id, key)
	}
	return nil
}

func AddUserKey(service_id string, app_id string, key string) error {
	url := INTERNAL_URL + "/services/" + service_id + "/applications/" + app_id + "/key" + key
	res, err := executeHttpRequest(http.MethodPut, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to add a user key(app_id: %s)", app_id)
	}
	return nil
}

func DeleteUserKey(service_id string, app_id string, key string) error {
	url := INTERNAL_URL + "/services/" + service_id + "/applications/key" + key
	res, err := executeHttpRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete a user key for app(id: %s)", app_id)
	}
	return nil
}

// Add a metrics to a service
func AddMetrics(service_id string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		header_data := []byte(fmt.Sprintf(`
			{ 
				"metric": {
					"service_id": "%s",
					"id": "%s",
					"name": "%s"
				}
			}`, service_id, metric.id, metric.name))
		url := INTERNAL_URL + "/services" + service_id + "/metrics/" + metric.id
		res, err := executeHttpRequest(http.MethodPost, url, &header_data)
		if err != nil {
			return err
		}
		if res.StatusCode != 201 {
			return fmt.Errorf("Failed to add a metric to the service(id: %s)", service_id)
		}
	}
	return nil
}

func DeleteMetrics(service_id string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		url := INTERNAL_URL + "/services/" + service_id + "/metrics/" + metric.id
		res, err := executeHttpRequest(http.MethodDelete, url, nil)
		if err != nil {
			return err
		}
		if res.StatusCode != 200 {
			return fmt.Errorf("Failed to delete the metric(service_id: %s, metric: %s)", service_id, metric.name)
		}
	}
	return nil
}

func UpdateUsageLimit(service_id string, plan_id string, metric_id string, limit UsageLimit) error {
	header_data := []byte(fmt.Sprintf(`
		{ 
			"usagelimit": {
				"%s": "%d"
			}
		}`, limit.period.String(), limit.value))
	url := INTERNAL_URL + "/services" + service_id + "/plans/" + plan_id + "/usagelimits/" + metric_id + "/" + limit.period.String()
	res, err := executeHttpRequest(http.MethodPut, url, &header_data)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to update usage limits for a metric(id: %s)", metric_id)
	}
	return nil
}

func UpdateUsageLimits(service_id string, plan_id string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		for _, limit := range metric.limits {
			if err := UpdateUsageLimit(service_id, plan_id, metric.id, limit); err != nil {
				return err
			}
		}
	}
	return nil
}

func DeleteUsageLimit(service_id string, plan_id string, metric_id string, period Period) error {
	url := INTERNAL_URL + "/services" + service_id + "/plans/" + plan_id + "/usagelimits/" + metric_id + "/" + period.String()
	res, err := executeHttpRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return errors.New("Failed to delete usage limits for a metric")
	}
	return nil
}

func DeleteUsageLimits(service_id string, plan_id string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		for _, limit := range metric.limits {
			if err := DeleteUsageLimit(service_id, plan_id, metric.id, limit.period); err != nil {
				return err
			}
		}
	}
	return nil
}

func executeHttpRequest(method string, url string, data *[]byte) (*http.Response, error) {
	client := &http.Client{}
	var body io.Reader
	if data != nil {
		body = bytes.NewBuffer(*data)
	}
	req, err := http.NewRequest(method, url, body)
	if err != nil {
		fmt.Printf("Error while creating HTTP request: %v", err)
		return nil, err
	}
	res, err := client.Do(req)
	if err != nil {
		fmt.Printf("Error sending the HTTP request: %v", err)
		return nil, err
	}
	defer res.Body.Close()
	return res, nil
}
