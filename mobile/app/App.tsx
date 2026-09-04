import { StatusBar } from 'expo-status-bar';
import { StyleSheet, Text, View } from 'react-native';

/**
 * Mobile app shell.
 *
 * Pairing, session list, and the live session view are added by the User
 * Story 2 and 3 tasks.
 */
export default function App() {
  return (
    <View style={styles.container}>
      <Text>T-ide</Text>
      <Text>Pair this device with your computer to get started.</Text>
      <StatusBar style="auto" />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
  },
});
