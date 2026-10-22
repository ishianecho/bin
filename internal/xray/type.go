package xray

type Node struct {
	Tag       string `json:"tag"`
	Address   string `json:"address"`
	Port      int    `json:"port"`
	UUID      string `json:"uuid"`
	SNI       string `json:"sni"`
	PublicKey string `json:"publicKey"`
	ShortID   string `json:"shortId"`
	Security  string `json:"security,omitempty"`
	Pin       string `json:"pin,omitempty"`
}

type Route struct {
	Action  string `json:"action"`
	Matcher string `json:"matcher"`
}

type Ports struct {
	Socks  int `json:"socks"`
	Tproxy int `json:"tproxy"`
}

type State struct {
	Version int     `json:"version"`
	Active  string  `json:"active"`
	Ports   Ports   `json:"ports"`
	Nodes   []Node  `json:"nodes"`
	Routes  []Route `json:"routes"`
}
