package xray

import (
	"encoding/json"
	"fmt"
	"slices"

	"github.com/google/uuid"
	"github.com/spf13/cobra"
)

func completeStateValues(pick func(*State) []string) cobra.CompletionFunc {
	return func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
		state, err := Load()
		if err != nil {
			return nil, cobra.ShellCompDirectiveError
		}
		var values []string
		for _, value := range pick(state) {
			if !slices.Contains(args, value) {
				values = append(values, value)
			}
		}
		return values, cobra.ShellCompDirectiveNoFileComp
	}
}

var completeNodeTags = completeStateValues(func(state *State) []string {
	tags := make([]string, len(state.Nodes))
	for i, node := range state.Nodes {
		tags[i] = node.Tag
	}
	return tags
})

var completeRouteMatchers = completeStateValues(func(state *State) []string {
	matchers := make([]string, len(state.Routes))
	for i, route := range state.Routes {
		matchers[i] = route.Matcher
	}
	return matchers
})

func isUUID(value string) bool {
	_, err := uuid.Parse(value)
	return err == nil
}

func printJSON(value any) {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return
	}
	fmt.Println(string(data))
}
